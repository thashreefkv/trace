import { icons, Target } from "lucide-react";

export const AVAILABLE_ICONS = icons as Record<string, React.FC<any>>;
export type IconName = keyof typeof icons;

export const DEFAULT_MEDICAL_ICONS: IconName[] = [
  // Medical
  "Activity", "Bandage", "Bone", "Brain", "BriefcaseMedical", "Capsule", "Cross", 
  "Dna", "Droplet", "Ear", "Eye", "FileHeart", "FlaskConical", "FlaskRound", 
  "Heart", "HeartPulse", "Hospital", "Microscope", "Pill", "Pills", "Radiation", 
  "Stethoscope", "Syringe", "Tablet", "Thermometer", "Lungs", "Baby", 
  "ClipboardPlus", "FolderHeart", "MonitorHeart", "ShieldPlus", "TestTube", 
  "TestTubes", "Virus",
  
  // Education & Digital
  "Book", "BookOpen", "BookCopy", "BookMarked", "GraduationCap", "Library", 
  "School", "Scroll", "University", "Notebook", "NotebookPen", "Presentation", 
  "Apple", "Award", "Backpack", "Bell", "Calculator", "Calendar", "Compass", 
  "Eraser", "Globe", "Languages", "Paperclip", "Pencil", "PenTool", "Puzzle", 
  "Quote", "Shapes", "Table", "Timer", "Trophy", "Video", "Wand2", "Webcam",
  "Monitor", "Laptop", "Smartphone", "Wifi", "Cloud", "MousePointer2", "ScreenShare",
  
  // Tests & AI
  "CheckSquare", "FileCheck", "FileEdit", "ClipboardCheck", "Pen", "CheckCircle2",
  "FileQuestion", "FileSignature", "AlarmClock",
  "Cpu", "Bot", "Zap", "Sparkles", "BrainCircuit", "CircuitBoard", "Atom", "Search", "Code2"
] as IconName[];

interface InitiativeIconProps {
  name: string;
  color?: string;
  size?: number;
  className?: string;
}

export function InitiativeIcon({ name, color = "#6366f1", size = 16, className = "" }: InitiativeIconProps) {
  const IconComponent = AVAILABLE_ICONS[name as IconName] || Target;
  
  return (
    <IconComponent 
      color={color} 
      size={size} 
      className={className} 
    />
  );
}
