import type { ReactNode } from "react";

/** One thing running now: what it is, in words, and how to stop it, where it can be. */
export type Activity = {
  key: string;
  words: ReactNode;
  stop?: { label: string; onStop: () => void };
};

/**
 * What is running now, one entry each, at the top of the page however far
 * it is scrolled: a strategy or an experiment playing, a package loading, a
 * ref followed, a history played back. With nothing running it says where
 * the store is kept.
 */
export function ActivityStrip({ activities, idle }: { activities: Activity[]; idle: string }) {
  return <section className="activity-strip" aria-label="Activity">
    {activities.length === 0 ? <span className="activity-idle">{idle}</span>
      : activities.map((activity) => <span className="activity-entry" key={activity.key}>
        <span className="activity-pulse" aria-hidden="true" />
        <span className="activity-words">{activity.words}</span>
        {activity.stop && <button type="button" onClick={activity.stop.onStop}>{activity.stop.label}</button>}
      </span>)}
  </section>;
}
