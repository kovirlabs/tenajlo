import { StartupGate } from "./components/StartupGate";
import { EmptyShell } from "./views/EmptyShell";

export function App() {
  return (
    <StartupGate>
      <EmptyShell />
    </StartupGate>
  );
}
