import { useEffect, useRef, useState } from "react";

const THINKING_VERBS = [
  // Kerala kitchen
  "Aviyaling", "Appaming", "Payasaming", "Puttuing", "Kappaying",
  "Rasaming", "Sambaring", "Karimeeening", "Chutnifying", "Filtercoffeeing",
  // Tamil Nadu
  "Dosaing", "Kulambuving", "Settifying", "Managifying", "Paathukalaming",
  "Rombaing", "Solvening", "Nanbaing", "Machaaing", "Kashtapaduting",
  // Malayalam slang
  "Aiyoing", "Adipoling", "Enthaing", "Paksheing", "Sheriyaaing",
  "Thattukadaing", "Chettaning", "Machaning", "Evidaing", "Athupoleing",
  // Hindi / pan-desi
  "Jugaading", "Gyaaning", "Fundaing", "Timepassaing", "Dekhteing",
  "Yaaraing", "Hojaaing", "Chaaing", "Jhakaassing", "Bakwasing",
  // Bangalore
  "Gottillaing", "Swalpaing", "Houdumaading", "Koramangalaing", "Startupifying",
  "OKRing", "Nammametroing", "BMTCing", "Signalmaading", "Deckmaarding",
];

const SCRAMBLE_CHARS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

function FlipWord({ word }: { word: string }) {
  const [chars, setChars] = useState<string[]>(() => word.split(""));
  const prevWord = useRef(word);
  const frameRef = useRef(0);

  useEffect(() => {
    if (prevWord.current === word) return;
    prevWord.current = word;
    const target = word.split("");
    frameRef.current = 0;
    const totalFrames = target.length * 3 + 6;
    const id = setInterval(() => {
      const f = ++frameRef.current;
      setChars(
        target.map((ch, i) => {
          if (f >= i * 3 + 4) return ch;
          return SCRAMBLE_CHARS[Math.floor(Math.random() * SCRAMBLE_CHARS.length)];
        }),
      );
      if (f >= totalFrames) clearInterval(id);
    }, 38);
    return () => clearInterval(id);
  }, [word]);

  return (
    <span>
      {chars.map((ch, i) => {
        const settled = ch === word[i];
        return (
          <span key={i} className={settled ? "text-orange-500" : "text-orange-200"}>
            {ch}
          </span>
        );
      })}
    </span>
  );
}

export function ThinkingTicker() {
  const [word, setWord] = useState(
    () => THINKING_VERBS[Math.floor(Math.random() * THINKING_VERBS.length)],
  );

  useEffect(() => {
    let id: ReturnType<typeof setTimeout>;
    const schedule = () => {
      id = setTimeout(
        () => {
          setWord((prev) => {
            let next: string;
            do {
              next = THINKING_VERBS[Math.floor(Math.random() * THINKING_VERBS.length)];
            } while (next === prev);
            return next;
          });
          schedule();
        },
        2000 + Math.random() * 600,
      );
    };
    schedule();
    return () => clearTimeout(id);
  }, []);

  return (
    <span className="inline-flex items-center gap-3">
      <span className="relative flex h-2.5 w-2.5 shrink-0">
        <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-orange-400 opacity-50" />
        <span className="relative inline-flex h-2.5 w-2.5 rounded-full bg-orange-500" />
      </span>
      <span className="inline-flex items-center gap-1 text-[15px] font-medium">
        <FlipWord word={word} />
        <span className="animate-pulse text-orange-300">▌</span>
      </span>
    </span>
  );
}
