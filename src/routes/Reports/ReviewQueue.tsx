import type { AgentReviewItem, ClaimReviewItem, CommunityReport } from "../../lib/types";

export function ReviewQueue({
  claims,
  communities,
  proposals,
  onReviewClaim,
  onReviewCommunity,
  onReviewProposal,
}: {
  claims: ClaimReviewItem[];
  communities: CommunityReport[];
  proposals: AgentReviewItem[];
  onReviewClaim: (id: string, decision: "approved" | "rejected") => Promise<void>;
  onReviewCommunity: (id: string, decision: "approved" | "rejected") => Promise<void>;
  onReviewProposal: (id: string, decision: "approved" | "rejected") => Promise<void>;
}) {
  return (
    <section className="rounded-2xl border border-zinc-200 bg-white p-4">
      <h2 className="text-sm font-semibold text-zinc-950">Reasoning review queue</h2>
      <p className="mt-1 text-[11px] leading-5 text-zinc-500">
        Pending claims and synthesis are not reusable graph context until approved.
      </p>
      {claims.length === 0 && communities.length === 0 && proposals.length === 0 ? (
        <p className="mt-4 text-xs text-zinc-400">No pending reasoning artifacts.</p>
      ) : null}
      {claims.slice(0, 5).map((claim) => (
        <ReviewCard
          body={claim.statement}
          key={claim.id}
          label={`Claim · ${Math.round(claim.confidence * 100)}%`}
          onApprove={() => void onReviewClaim(claim.id, "approved")}
          onReject={() => void onReviewClaim(claim.id, "rejected")}
        />
      ))}
      {communities.slice(0, 3).map((report) => (
        <ReviewCard
          body={report.summary_markdown.slice(0, 220)}
          key={report.id}
          label="Community synthesis"
          onApprove={() => void onReviewCommunity(report.id, "approved")}
          onReject={() => void onReviewCommunity(report.id, "rejected")}
        />
      ))}
      {proposals.slice(0, 5).map((proposal) => (
        <ReviewCard
          body={proposal.proposed_change_json}
          key={proposal.id}
          label={`Action proposal · ${proposal.proposal_type}`}
          onApprove={() => void onReviewProposal(proposal.id, "approved")}
          onReject={() => void onReviewProposal(proposal.id, "rejected")}
        />
      ))}
    </section>
  );
}

function ReviewCard({
  label,
  body,
  onApprove,
  onReject,
}: {
  label: string;
  body: string;
  onApprove: () => void;
  onReject: () => void;
}) {
  return (
    <article className="mt-3 rounded-xl border border-zinc-100 p-3">
      <div className="text-[10px] font-semibold uppercase tracking-wide text-violet-600">{label}</div>
      <p className="mt-1 line-clamp-4 text-xs leading-5 text-zinc-700">{body}</p>
      <div className="mt-2 flex gap-2">
        <button className="rounded-md bg-zinc-950 px-2 py-1 text-[11px] text-white" onClick={onApprove} type="button">Approve</button>
        <button className="rounded-md border border-zinc-200 px-2 py-1 text-[11px] text-zinc-600" onClick={onReject} type="button">Reject</button>
      </div>
    </article>
  );
}
