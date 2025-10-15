import { Header } from "./Header";
import { Form } from "./Form";

export function Content() {
  return (
    <div style="position: fixed; top: 0; left: 0; width: 100vw; height: 100vh; z-index: 30; pointer-events: none; display: flex; align-items: center; justify-content: center;">
      <div style="background-color: #000; padding: 2rem; pointer-events: auto;">
        <div class="w-full max-w-4xl mx-auto px-4">
          <Header />
        </div>
      </div>
    </div>
  );
}
