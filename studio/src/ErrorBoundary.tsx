import { Component, type ReactNode } from "react";

type Props = {
  /** What its children are, as said when they fail to draw. */
  label: string;
  /** Children that failed are drawn again once this changes. */
  resetOn?: unknown;
  /** Recovers by reloading the page, for what holds the whole of it. */
  reload?: boolean;
  children: ReactNode;
};

type State = { failed: false } | { failed: true; cause: unknown };

/**
 * Draws its children or, when drawing them throws, what was thrown in their
 * place. Without one above it, anything that throws while drawing empties
 * the whole page.
 */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { failed: false };

  static getDerivedStateFromError(cause: unknown): State {
    return { failed: true, cause };
  }

  componentDidUpdate(previous: Props, was: State) {
    if (was.failed && this.state.failed && previous.resetOn !== this.props.resetOn) this.setState({ failed: false });
  }

  render() {
    if (!this.state.failed) return this.props.children;
    const { label, reload } = this.props;
    const { cause } = this.state;
    return <div className="render-failure" role="alert">
      <p><strong>{label} could not be drawn.</strong> {cause instanceof Error ? cause.message : String(cause)}</p>
      <button type="button" onClick={() => { if (reload) window.location.reload(); else this.setState({ failed: false }); }}>{reload ? "Reload" : "Try again"}</button>
    </div>;
  }
}
