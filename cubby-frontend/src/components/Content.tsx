import { Header } from "./Header";
import { Form } from "./Form";

export function Content() {
  return (
    <div style="position: fixed; top: 0; left: 0; width: 100vw; height: 100vh; z-index: 30; pointer-events: none; display: flex; align-items: center; justify-content: center;">
      <div style="background-color: #000; padding: 2rem; pointer-events: auto;">
        <div class="w-full max-w-4xl mx-auto px-4">
          <Header />
          
          <div class="flex items-stretch gap-0 mx-auto mb-8" style="max-width: 400px; margin-top: 1.5rem;">
            <div style="background-color: #111; border: 1px solid #333; padding: 0.5rem 0.875rem; flex: 1; font-family: 'Courier New', monospace; font-size: 0.875rem; user-select: all;">
              curl -fsSL https://get.cubby.sh/cli | sh
            </div>
            <button 
              onclick="navigator.clipboard.writeText('curl -fsSL https://get.cubby.sh/cli | sh'); this.style.backgroundColor='#fff'; this.style.color='#000'; setTimeout(() => { this.style.backgroundColor='#222'; this.style.color='#fff'; }, 75);"
              style="background-color: #222; color: #fff; border: 1px solid #333; border-left: none; padding: 0.5rem 1rem; cursor: pointer; font-family: 'Courier New', monospace; font-size: 0.875rem; transition: none;"
            >
              &#x2398;
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
