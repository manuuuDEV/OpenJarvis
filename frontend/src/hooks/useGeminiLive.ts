import { useCallback, useEffect, useRef, useState } from 'react';
import { isTauri, mintGeminiLiveSessionToken } from '../lib/api';

const GEMINI_LIVE_MODEL = 'gemini-3.1-flash-live-preview';
const GEMINI_LIVE_WS =
  'wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContentConstrained';
const INPUT_SAMPLE_RATE = 16_000;
const OUTPUT_SAMPLE_RATE = 24_000;

type LiveState = 'idle' | 'connecting' | 'listening' | 'error';

interface LiveSessionRefs {
  socket: WebSocket | null;
  stream: MediaStream | null;
  inputContext: AudioContext | null;
  outputContext: AudioContext | null;
  inputSource: MediaStreamAudioSourceNode | null;
  processor: ScriptProcessorNode | null;
  mutedGain: GainNode | null;
  nextPlaybackTime: number;
}

function encodeBase64(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  let binary = '';
  for (let index = 0; index < bytes.byteLength; index += 1) {
    binary += String.fromCharCode(bytes[index]);
  }
  return window.btoa(binary);
}

function decodeBase64(value: string): Uint8Array {
  const binary = window.atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

function toPcm16Mono(input: Float32Array, sourceRate: number): ArrayBuffer {
  const ratio = sourceRate / INPUT_SAMPLE_RATE;
  const outputLength = Math.max(1, Math.round(input.length / ratio));
  const output = new Int16Array(outputLength);
  for (let index = 0; index < outputLength; index += 1) {
    const sourceIndex = Math.min(input.length - 1, Math.floor(index * ratio));
    const sample = Math.max(-1, Math.min(1, input[sourceIndex] ?? 0));
    output[index] = Math.round(sample * 0x7fff);
  }
  return output.buffer;
}

/**
 * Browser-side half of Gemini Live. The native Tauri command creates a
 * one-use, model-constrained token; this hook keeps it in memory only and
 * streams microphone PCM directly to Google over WSS.
 */
export function useGeminiLive() {
  const [state, setState] = useState<LiveState>('idle');
  const [error, setError] = useState('');
  const refs = useRef<LiveSessionRefs>({
    socket: null,
    stream: null,
    inputContext: null,
    outputContext: null,
    inputSource: null,
    processor: null,
    mutedGain: null,
    nextPlaybackTime: 0,
  });

  const stop = useCallback(() => {
    const current = refs.current;
    current.socket?.close(1000, 'User ended Gemini Live session');
    current.socket = null;
    current.processor?.disconnect();
    current.inputSource?.disconnect();
    current.mutedGain?.disconnect();
    current.stream?.getTracks().forEach((track) => track.stop());
    if (current.inputContext && current.inputContext.state !== 'closed') {
      void current.inputContext.close();
    }
    if (current.outputContext && current.outputContext.state !== 'closed') {
      void current.outputContext.close();
    }
    refs.current = {
      socket: null,
      stream: null,
      inputContext: null,
      outputContext: null,
      inputSource: null,
      processor: null,
      mutedGain: null,
      nextPlaybackTime: 0,
    };
    setState('idle');
  }, []);

  const playPcm16 = useCallback(async (base64Audio: string) => {
    const current = refs.current;
    if (!current.outputContext) {
      current.outputContext = new AudioContext({ sampleRate: OUTPUT_SAMPLE_RATE });
    }
    const context = current.outputContext;
    if (context.state === 'suspended') await context.resume();
    const bytes = decodeBase64(base64Audio);
    const samples = new Int16Array(bytes.buffer, bytes.byteOffset, Math.floor(bytes.byteLength / 2));
    const audioBuffer = context.createBuffer(1, samples.length, OUTPUT_SAMPLE_RATE);
    const channel = audioBuffer.getChannelData(0);
    for (let index = 0; index < samples.length; index += 1) {
      channel[index] = samples[index] / 32768;
    }
    const source = context.createBufferSource();
    source.buffer = audioBuffer;
    source.connect(context.destination);
    const startAt = Math.max(context.currentTime, current.nextPlaybackTime);
    source.start(startAt);
    current.nextPlaybackTime = startAt + audioBuffer.duration;
  }, []);

  const startMicrophone = useCallback(async () => {
    const current = refs.current;
    if (!current.socket || current.socket.readyState !== WebSocket.OPEN) return;
    const stream = await navigator.mediaDevices.getUserMedia({
      audio: {
        echoCancellation: true,
        noiseSuppression: true,
        autoGainControl: true,
        channelCount: 1,
      },
      video: false,
    });
    const context = new AudioContext({ sampleRate: INPUT_SAMPLE_RATE });
    if (context.state === 'suspended') await context.resume();
    const source = context.createMediaStreamSource(stream);
    // ScriptProcessor is supported by the desktop WebView and keeps this
    // implementation self-contained; input is converted to PCM16 immediately.
    const processor = context.createScriptProcessor(2048, 1, 1);
    const mutedGain = context.createGain();
    mutedGain.gain.value = 0;
    processor.onaudioprocess = (event) => {
      const socket = refs.current.socket;
      if (!socket || socket.readyState !== WebSocket.OPEN) return;
      const pcm = toPcm16Mono(event.inputBuffer.getChannelData(0), context.sampleRate);
      socket.send(JSON.stringify({
        realtimeInput: {
          audio: {
            data: encodeBase64(pcm),
            mimeType: 'audio/pcm;rate=16000',
          },
        },
      }));
    };
    source.connect(processor);
    processor.connect(mutedGain);
    mutedGain.connect(context.destination);
    current.stream = stream;
    current.inputContext = context;
    current.inputSource = source;
    current.processor = processor;
    current.mutedGain = mutedGain;
    setState('listening');
  }, []);

  const start = useCallback(async () => {
    if (!isTauri()) {
      setError('Gemini Live è disponibile soltanto nell’app desktop.');
      setState('error');
      return;
    }
    if (refs.current.socket) return;
    setError('');
    setState('connecting');
    try {
      const token = await mintGeminiLiveSessionToken();
      // The temporary token is used only in this in-memory URL. It is never
      // saved, logged, attached to a chat message, or sent to the Python API.
      const socket = new WebSocket(`${GEMINI_LIVE_WS}?access_token=${encodeURIComponent(token.accessToken)}`);
      refs.current.socket = socket;
      socket.onopen = () => {
        socket.send(JSON.stringify({
          setup: {
            model: `models/${GEMINI_LIVE_MODEL}`,
            responseModalities: ['AUDIO'],
            realtimeInputConfig: { automaticActivityDetection: {} },
            inputAudioTranscription: {},
            outputAudioTranscription: {},
          },
        }));
      };
      socket.onmessage = async (event) => {
        try {
          const raw = typeof event.data === 'string' ? event.data : await event.data.text();
          const message = JSON.parse(raw);
          // Live audio has no local tool bridge. Close rather than let a model
          // turn a real-time session into an unapproved automation channel.
          if (message.toolCall) {
            setError('Gemini Live ha richiesto un tool non autorizzato; sessione chiusa.');
            stop();
            return;
          }
          if (message.setupComplete) {
            await startMicrophone();
            return;
          }
          const parts = message.serverContent?.modelTurn?.parts;
          if (Array.isArray(parts)) {
            for (const part of parts) {
              if (part.inlineData?.data) await playPcm16(part.inlineData.data);
            }
          }
        } catch {
          setError('Risposta Gemini Live non valida; sessione chiusa.');
          stop();
        }
      };
      socket.onerror = () => {
        setError('Connessione Gemini Live non riuscita. Controlla rete, chiave e accesso al modello.');
        stop();
      };
      socket.onclose = () => {
        if (refs.current.socket === socket) stop();
      };
    } catch (cause: any) {
      setError(cause?.message ?? 'Impossibile avviare Gemini Live.');
      stop();
    }
  }, [playPcm16, startMicrophone, stop]);

  useEffect(() => () => stop(), [stop]);

  return {
    state,
    error,
    isLive: state === 'listening',
    isConnecting: state === 'connecting',
    start,
    stop,
  };
}
