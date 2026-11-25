import type { ReactNode } from "react";

/** One thing running now: what it is, in words, and how to stop it, where it can be. */
export type Activity = {
  key: string;
  words: ReactNode;
  stop?: { label: string; onStop: () => void };
};

/** What the last thing done came to: done as asked, done otherwise, or failed. */
export type Said = { tone: "done" | "caution" | "error"; message: string };

/** Where the store is kept, in a few words, and why it is not, when it is not. */
export type StoreWords = { words: string; why?: string };

/**
 * What is running now, one entry each, in the bar at the top of the page: a
 * strategy or an experiment playing, a package loading, a ref followed, a
 * timeline played back. Then what the last thing done came to, until the next
 * one, and where the store is kept, with why it is not on its hover.
 */
export function ActivityStrip({ activities, said, store, onReplaceStore }: { activities: Activity[]; said: Said[]; store: StoreWords; onReplaceStore?: () => void }) {
  const words = said.map((each) => each.message).join(" ");
  const tone = said.find((each) => each.tone === "error")?.tone ?? said.find((each) => each.tone === "caution")?.tone ?? "done";
  return <section className="activity-strip" aria-label="Activity">
    {activities.map((activity) => <span className="activity-entry" key={activity.key}>
      <span className="activity-pulse" aria-hidden="true" />
      <span className="activity-words">{activity.words}</span>
      {activity.stop && <button type="button" onClick={activity.stop.onStop}>{activity.stop.label}</button>}
    </span>)}
    {words && <span className={`activity-said tone-${tone}`} role={tone === "error" ? "alert" : "status"} title={words}>{words}</span>}
    <span className="activity-store">{store.why ? <span className="store-why" tabIndex={0} title={store.why} aria-label={`${store.words}: ${store.why}`}>{store.words}</span> : store.words}{onReplaceStore && <button type="button" onClick={onReplaceStore} title="Replace the saved store with an empty one; what it held is lost">Replace it</button>}</span>
  </section>;
}
