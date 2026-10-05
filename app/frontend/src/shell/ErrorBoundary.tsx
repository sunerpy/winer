import { TriangleAlert } from "lucide-react";
import { Component, type ErrorInfo, type ReactNode } from "react";

import { Button, EmptyState } from "../ui";

interface Props {
  title: string;
  hint: string;
  retryLabel: string;
  logsLabel: string;
  onOpenLogs: () => void;
  children: ReactNode;
}

/** Contains a crash to the page that had it: the sidebar, title bar and settings stay usable. */
export class ErrorBoundary extends Component<Props, { error: Error | null }> {
  override state = { error: null as Error | null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  override componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("page crashed", error, info.componentStack);
  }

  override render() {
    if (!this.state.error) return this.props.children;
    return (
      <EmptyState
        icon={TriangleAlert}
        title={this.props.title}
        actions={
          <>
            <Button onClick={() => this.setState({ error: null })}>{this.props.retryLabel}</Button>
            <Button variant="ghost" onClick={this.props.onOpenLogs}>
              {this.props.logsLabel}
            </Button>
          </>
        }
      >
        <p>{this.props.hint}</p>
        <p className="mono mt-2 text-[11px] text-fg-subtle">{this.state.error.message}</p>
      </EmptyState>
    );
  }
}
