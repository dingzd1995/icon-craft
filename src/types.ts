export type TargetKind = "folder" | "extension" | "file";

export interface IconRule {
  id: string;
  name: string;
  target: string;
  kind: TargetKind;
  iconPath: string;
  enabled: boolean;
  monitored?: boolean;
  lastApplied?: number;
  status?: string;
  backup?: string;
  recursiveMode?: "none" | "depth" | "all";
  maxDepth?: number;
  appliedTargets?: Array<{ target: string; backup?: string }>;
}

export interface AppSettings {
  monitorEnabled: boolean;
  monitorIntervalMinutes: number;
  autostart: boolean;
  rules: IconRule[];
  easterBatches?: EasterBatch[];
}

export interface EasterBatch {
  id: string;
  root: string;
  iconPath: string;
  createdAt: number;
  directoryCount: number;
  fileCount: number;
  extensions: string[];
}

export interface EasterPreview {
  directoryCount: number;
  fileCount: number;
  extensions: string[];
}
