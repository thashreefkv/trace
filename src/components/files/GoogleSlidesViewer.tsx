import { useEffect, useState } from "react";
import { AlertCircle, Loader2 } from "lucide-react";
import { driveHasEditorScope, getGoogleSlides, getSlideThumbnail } from "../../lib/files";

interface GoogleSlidesViewerProps {
  driveFileId: string;
}

interface SlideInfo {
  objectId: string;
  title?: string;
}

export function GoogleSlidesViewer({ driveFileId }: GoogleSlidesViewerProps) {
  const [slides, setSlides] = useState<SlideInfo[]>([]);
  const [thumbnails, setThumbnails] = useState<Record<string, string>>({});
  const [selected, setSelected] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [needsReauth, setNeedsReauth] = useState(false);

  useEffect(() => {
    void (async () => {
      try {
        setLoading(true);

        const hasScope = await driveHasEditorScope();
        if (!hasScope) {
          setNeedsReauth(true);
          return;
        }

        const pres = await getGoogleSlides(driveFileId) as {
          slides?: Array<{ objectId: string; slideProperties?: { name?: string } }>;
        };
        const slideList: SlideInfo[] = (pres.slides ?? []).map((s) => ({
          objectId: s.objectId,
          title: s.slideProperties?.name,
        }));
        setSlides(slideList);
        if (slideList[0]) setSelected(slideList[0].objectId);

        // Eagerly load first 5 thumbnails.
        await Promise.all(
          slideList.slice(0, 5).map(async (s) => {
            try {
              const url = await getSlideThumbnail(driveFileId, s.objectId);
              setThumbnails((prev) => ({ ...prev, [s.objectId]: url }));
            } catch {
              // ignore individual thumbnail failures
            }
          }),
        );
      } catch (e) {
        setError(String(e));
      } finally {
        setLoading(false);
      }
    })();
  }, [driveFileId]);

  async function loadThumbnail(objectId: string) {
    if (thumbnails[objectId]) return;
    try {
      const url = await getSlideThumbnail(driveFileId, objectId);
      setThumbnails((prev) => ({ ...prev, [objectId]: url }));
    } catch {
      // ignore
    }
  }

  if (needsReauth) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 p-8 text-center">
        <AlertCircle className="text-amber-500" size={28} />
        <p className="text-[13px] font-medium text-zinc-800">Re-connect Drive to view slides</p>
        <p className="max-w-[260px] text-[12px] text-zinc-500">
          Go to the Drive tab, disconnect, and reconnect your account to enable the Slides viewer.
        </p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex h-full items-center justify-center p-8 text-center text-[12px] text-rose-600">
        {error}
      </div>
    );
  }

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center gap-2 text-[12px] text-zinc-500">
        <Loader2 className="animate-spin" size={13} /> Loading presentation…
      </div>
    );
  }

  const activeThumbnail = selected ? thumbnails[selected] : null;

  return (
    <div className="flex h-full min-h-0">
      {/* Slide strip */}
      <div className="flex w-32 shrink-0 flex-col gap-1.5 overflow-y-auto border-r border-zinc-200 bg-zinc-50 p-1.5">
        {slides.map((slide, i) => (
          <button
            key={slide.objectId}
            className={[
              "relative overflow-hidden rounded border text-left",
              selected === slide.objectId
                ? "border-sky-500 ring-1 ring-sky-500"
                : "border-zinc-200 hover:border-zinc-300",
            ].join(" ")}
            onClick={() => {
              setSelected(slide.objectId);
              void loadThumbnail(slide.objectId);
            }}
            type="button"
          >
            {thumbnails[slide.objectId] ? (
              <img alt={`Slide ${i + 1}`} className="w-full" src={thumbnails[slide.objectId]} />
            ) : (
              <div className="flex aspect-video w-full items-center justify-center bg-zinc-100 text-[10px] text-zinc-400">
                {i + 1}
              </div>
            )}
          </button>
        ))}
      </div>

      {/* Main slide view */}
      <div className="flex min-w-0 flex-1 items-center justify-center overflow-auto bg-zinc-100 p-4">
        {activeThumbnail ? (
          <img
            alt="Current slide"
            className="max-h-full max-w-full rounded-md shadow-sm"
            src={activeThumbnail}
          />
        ) : (
          <div className="flex items-center gap-2 text-[12px] text-zinc-500">
            <Loader2 className="animate-spin" size={13} /> Loading slide…
          </div>
        )}
      </div>
    </div>
  );
}
