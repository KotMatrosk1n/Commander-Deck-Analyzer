import {
  Component,
  type ErrorInfo,
  type ReactNode,
} from "react";

export interface UiErrorFallbackProps {
  error: Error;
  reset: () => void;
}

interface UiErrorBoundaryProps {
  children: ReactNode;
  fallback: (props: UiErrorFallbackProps) => ReactNode;
}

interface UiErrorBoundaryState {
  error: Error | null;
}

export class UiErrorBoundary extends Component<
  UiErrorBoundaryProps,
  UiErrorBoundaryState
> {
  state: UiErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: unknown): UiErrorBoundaryState {
    return {
      error: error instanceof Error ? error : new Error(String(error)),
    };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("Commander Deck Analyzer UI render failure", error, info.componentStack);
  }

  private reset = () => {
    this.setState({ error: null });
  };

  render() {
    if (this.state.error) {
      return this.props.fallback({
        error: this.state.error,
        reset: this.reset,
      });
    }

    return this.props.children;
  }
}
