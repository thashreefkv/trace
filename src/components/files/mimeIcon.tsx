import {
  FileAudio,
  FileCode,
  FileImage,
  FileSpreadsheet,
  FileText,
  FileType,
  FileVideo,
  File as FileIcon,
  Presentation,
} from "lucide-react";
import type { ReactNode } from "react";

export function mimeIcon(mime?: string | null, size = 14): ReactNode {
  const m = (mime ?? "").toLowerCase();
  if (m.includes("application/vnd.google-apps.document")) {
    return <FileText className="text-sky-600" size={size} />;
  }
  if (m.includes("application/vnd.google-apps.spreadsheet")) {
    return <FileSpreadsheet className="text-emerald-600" size={size} />;
  }
  if (m.includes("application/vnd.google-apps.presentation")) {
    return <Presentation className="text-amber-600" size={size} />;
  }
  if (m.startsWith("image/")) return <FileImage className="text-violet-600" size={size} />;
  if (m.startsWith("video/")) return <FileVideo className="text-rose-600" size={size} />;
  if (m.startsWith("audio/")) return <FileAudio className="text-orange-600" size={size} />;
  if (m === "application/pdf") return <FileType className="text-red-600" size={size} />;
  if (
    m.includes("officedocument.wordprocessingml") ||
    m === "application/msword"
  ) {
    return <FileText className="text-blue-600" size={size} />;
  }
  if (
    m.includes("officedocument.spreadsheetml") ||
    m === "application/vnd.ms-excel"
  ) {
    return <FileSpreadsheet className="text-green-600" size={size} />;
  }
  if (
    m.includes("officedocument.presentationml") ||
    m === "application/vnd.ms-powerpoint"
  ) {
    return <Presentation className="text-orange-500" size={size} />;
  }
  if (m === "text/plain" || m === "text/markdown") {
    return <FileText className="text-zinc-600" size={size} />;
  }
  if (m.startsWith("text/") || m.includes("json") || m.includes("xml")) {
    return <FileCode className="text-zinc-600" size={size} />;
  }
  return <FileIcon className="text-zinc-500" size={size} />;
}

export function humanSize(bytes?: number | null): string {
  if (bytes == null) return "";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}
