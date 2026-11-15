import type { ReactNode } from "react";

/** One thing running now: what it is, in words, and how to stop it, where it can be. */
export type Activity = {
  key: string;
  words: ReactNode;
  stop?: { label: string; onStop: () => void };
};

/** What the last thing done came to: done as asked, done otherwise, or failed. */
export type Said = { tone: "done" | "caution" | "error"; message: string };

/**
 * What is running now, one entry each, at the top of the page however far
 * it is scrolled: a strategy or an experiment playing, a package loading, a
 * ref followed, a history played back. Then what the last thing done came
 * to, until the next one, and where the store is kept.
 */
export function ActivityStrip({ activities, said, store, onReplaceStore }: { activities: Activity[]; said: Said[]; store: string; onReplaceStore?: () => void }) {
  const words = said.map((each) => each.message).join(" ");
  const tone = said.find((each) => each.tone === "error")?.tone ?? said.find((each) => each.tone === "caution")?.tone ?? "done";
  return <section className="activity-strip" aria-label="Activity">
    {activities.map((activity) => <span className="activity-entry" key={activity.key}>
      <span className="activity-pulse" aria-hidden="true" />
      <span className="activity-words">{activity.words}</span>
      {activity.stop && <button type="button" onClick={activity.stop.onStop}>{activity.stop.label}</button>}
    </span>)}
    {words && <span className={`activity-said tone-${tone}`} role={tone === "error" ? "alert" : "status"} title={words}>{words}</span>}
    <span className="activity-store">{store}{onReplaceStore && <button type="button" onClick={onReplaceStore} title="Replace the saved store with an empty one; what it held is lost">Replace it</button>}</span>
  </section>;
}
