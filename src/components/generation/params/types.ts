export interface Wan27FormValues {
  prompt: string;
  negative_prompt?: string;
  n: number;
  size: "1024*1024" | "2K";
  enable_sequential: boolean;
}

export interface CosyVoiceFormValues {
  text: string;
  voice_id: string;
  rate: number;
}

export interface HappyhorseFormValues {
  prompt: string;
  subject_ids: string[];
  resolution: "480P" | "720P" | "1080P";
  ratio: "16:9" | "9:16" | "1:1";
  duration: 3 | 5 | 10;
}
