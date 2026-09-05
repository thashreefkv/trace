import {
  BarChart3,
  Braces,
  FileCode2,
  FileText,
  FileType2,
  FlaskConical,
  GitBranch,
  Mail,
  MessageSquareText,
  Network,
  Newspaper,
  Presentation,
  Route,
  Search,
  Shapes,
} from "lucide-react";
import type { DeliverableType } from "../lib/types";
import { deliverableTypeLabels } from "../lib/types";

const iconByType = {
  deck: Presentation,
  design_doc: FileText,
  prototype: FlaskConical,
  analysis: BarChart3,
  framework: Network,
  pitch: MessageSquareText,
  research: Search,
  code: FileCode2,
  email: Mail,
  meeting_prep: Braces,
  spec: FileType2,
  report: Newspaper,
  roadmap: Route,
  brief: GitBranch,
  plan: Route,
  other: Shapes,
} satisfies Record<DeliverableType, typeof Presentation>;

interface DeliverableTypeBadgeProps {
  type: DeliverableType;
}

export function DeliverableTypeBadge({ type }: DeliverableTypeBadgeProps) {
  const Icon = iconByType[type];

  return (
    <span className="inline-flex items-center gap-1.5 text-[10px] font-bold uppercase tracking-widest text-zinc-400 dark:text-zinc-500">
      <Icon aria-hidden="true" size={13} strokeWidth={2} />
      {deliverableTypeLabels[type]}
    </span>
  );
}
