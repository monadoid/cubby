export function Fluid() {
  return (
    <>
      <canvas id="canvas" />
      <div id="render" class="render" />
      <script src="/fluid-init.js" />
      
      <style dangerouslySetInnerHTML={{
        __html: `
          * {
            user-select: none;
          }

          :root {
            --cell-size: 8px;
          }

          canvas {
            z-index: 1;
            image-rendering: pixelated;
            position: fixed;
            top: 0;
            left: 0;
            width: 100vw;
            height: 100vh;
            pointer-events: auto;
          }

          .render {
            position: fixed;
            top: 0;
            left: 0;
            width: 100vw;
            height: 100vh;
            white-space: pre;
            letter-spacing: 0.4em;
            font-size: var(--cell-size);
            line-height: var(--cell-size);
            font-weight: 700;
            font-family: "Geist Mono", monospace;
            z-index: 2;
            pointer-events: none; /* let clicks pass through to the canvas */
          }
        `
      }} />
    </>
  );
}
