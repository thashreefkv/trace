import { useEffect, useState } from "react";
import {
  CheckCircle2,
  Loader2,
  User,
  Mail,
  Briefcase,
  PenTool,
  Save,
} from "lucide-react";
import { getUserProfile, updateUserProfile } from "../lib/ipc";
import type { UserProfile } from "../lib/types";

export function MyProfile() {
  const [profile, setProfile] = useState<UserProfile | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);

  const [name, setName] = useState("");
  const [role, setRole] = useState("");
  const [email, setEmail] = useState("");
  const [bio, setBio] = useState("");

  useEffect(() => {
    void load();
  }, []);

  async function load() {
    setLoading(true);
    try {
      const p = await getUserProfile();
      setProfile(p);
      setName(p.name);
      setRole(p.role ?? "");
      setEmail(p.email ?? "");
      setBio(p.bio ?? "");
    } catch (e) {
      setMessage({ ok: false, text: String(e) });
    } finally {
      setLoading(false);
    }
  }

  async function handleSave() {
    if (!name.trim()) return;
    setSaving(true);
    setMessage(null);
    try {
      const next = await updateUserProfile({
        name: name.trim(),
        role: role.trim() || null,
        email: email.trim() || null,
        bio: bio.trim() || null,
        avatar_url: profile?.avatar_url ?? null,
      });
      setProfile(next);
      setMessage({ ok: true, text: "Profile updated successfully." });
      setTimeout(() => setMessage(null), 3000);
    } catch (e) {
      setMessage({ ok: false, text: String(e) });
    } finally {
      setSaving(false);
    }
  }

  if (loading) {
    return (
      <div className="mx-auto max-w-2xl space-y-4 px-5 py-8">
        <div className="skeleton h-40 rounded-2xl" />
        <div className="skeleton h-32 rounded-2xl" />
        <div className="skeleton h-32 rounded-2xl" />
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-2xl px-5 py-8 space-y-8">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold tracking-tight text-zinc-950">My Profile</h1>
        <button
          className="btn btn-primary shadow-sm"
          disabled={saving || !name.trim()}
          onClick={() => void handleSave()}
          type="button"
        >
          {saving ? <Loader2 size={14} className="animate-spin" /> : <Save size={14} />}
          Save Changes
        </button>
      </div>

      {message && (
        <div
          className={[
            "flex items-center gap-2 rounded-lg px-3 py-2 text-[13px] transition-all",
            message.ok ? "bg-emerald-50 text-emerald-800" : "bg-red-50 text-red-700",
          ].join(" ")}
        >
          {message.ok ? <CheckCircle2 size={14} /> : <Save size={14} />}
          {message.text}
        </div>
      )}

      {/* ── Header Card ────────────────────────────────────────────────────── */}
      <section className="rounded-2xl border border-zinc-100 bg-white p-8 shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
        <div className="flex flex-col items-center gap-6 text-center sm:flex-row sm:text-left">
          <div className="flex h-24 w-24 shrink-0 items-center justify-center rounded-3xl bg-zinc-50 border border-zinc-200 text-zinc-300 shadow-inner">
            <User size={40} strokeWidth={1.5} />
          </div>
          <div className="flex-1 space-y-1">
            <h2 className="text-2xl font-bold tracking-tight text-zinc-950">
              {name || "Your Name"}
            </h2>
            <p className="text-sm font-medium text-zinc-500">
              {role || "Add your role..."}
            </p>
            <div className="flex flex-wrap justify-center gap-4 pt-2 sm:justify-start">
              {email && (
                <div className="flex items-center gap-1.5 text-[12px] text-zinc-400">
                  <Mail size={13} />
                  {email}
                </div>
              )}
              <div className="flex items-center gap-1.5 text-[12px] text-zinc-400">
                <CheckCircle2 size={13} className="text-emerald-500" />
                Verified Workspace
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* ── Details Grid ────────────────────────────────────────────────────── */}
      <div className="grid gap-6 sm:grid-cols-2">
        {/* Basic Info */}
        <div className="space-y-4 rounded-2xl border border-zinc-100 bg-white p-6 shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
          <div className="flex items-center gap-2 border-b border-zinc-100 pb-3">
            <User size={16} className="text-zinc-400" />
            <h3 className="text-[13px] font-semibold text-zinc-900">Basic Information</h3>
          </div>
          <div className="space-y-4">
            <div className="space-y-1.5">
              <label className="field-label">Full Name</label>
              <input
                type="text"
                className="field-control"
                placeholder="e.g. Dr. Sarah Chen"
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            </div>
            <div className="space-y-1.5">
              <label className="field-label">Primary Email</label>
              <input
                type="email"
                className="field-control"
                placeholder="sarah@example.com"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
              />
            </div>
          </div>
        </div>

        {/* Professional Info */}
        <div className="space-y-4 rounded-2xl border border-zinc-100 bg-white p-6 shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
          <div className="flex items-center gap-2 border-b border-zinc-100 pb-3">
            <Briefcase size={16} className="text-zinc-400" />
            <h3 className="text-[13px] font-semibold text-zinc-900">Professional Identity</h3>
          </div>
          <div className="space-y-4">
            <div className="space-y-1.5">
              <label className="field-label">Role / Title</label>
              <input
                type="text"
                className="field-control"
                placeholder="e.g. Lead Educator"
                value={role}
                onChange={(e) => setRole(e.target.value)}
              />
            </div>
            <p className="text-[11px] text-zinc-400 leading-relaxed">
              This title will be used to contextualize your briefings and AI-generated roadmaps.
            </p>
          </div>
        </div>

        {/* Bio / About */}
        <div className="space-y-4 rounded-2xl border border-zinc-100 bg-white p-6 shadow-[0_2px_12px_rgba(0,0,0,0.06)] sm:col-span-2">
          <div className="flex items-center gap-2 border-b border-zinc-100 pb-3">
            <PenTool size={16} className="text-zinc-400" />
            <h3 className="text-[13px] font-semibold text-zinc-900">About Me</h3>
          </div>
          <div className="space-y-1.5">
            <label className="field-label">Biography</label>
            <textarea
              className="field-control min-h-[100px] resize-none py-2"
              placeholder="Tell Trace about your goals and focus..."
              value={bio}
              onChange={(e) => setBio(e.target.value)}
            />
          </div>
          <p className="text-[11px] text-zinc-400">
            A detailed bio helps the AI prioritize tasks and filter irrelevant emails based on your core focus.
          </p>
        </div>
      </div>

      <div className="pt-4 text-center">
        <p className="text-[11px] text-zinc-300">
          Last updated: {profile ? new Date(profile.updated_at).toLocaleString() : "Never"}
        </p>
      </div>
    </div>
  );
}
