export function TopBar() {
  return (
    <div style="position: fixed; top: 0; left: 0; width: 100vw; height: 48px; background-color: #000; border-bottom: 1px solid #333; z-index: 40; display: flex; align-items: center; justify-content: space-between; padding: 0 1rem;">
      <div style="display: flex; align-items: center;">
        <img 
          src="/cubby_logo_white.png" 
          alt="cubby logo" 
          style="height: 24px; width: auto; image-rendering: pixelated; image-rendering: -moz-crisp-edges; image-rendering: crisp-edges;"
        />
      </div>
      <div style="display: flex; gap: 1rem;">
        <a 
          href="/login" 
          style="color: #fff; text-decoration: none; font-family: 'Courier New', monospace; font-size: 14px; hover: color: #ccc;"
          onMouseOver={(e) => e.target.style.color = '#ccc'}
          onMouseOut={(e) => e.target.style.color = '#fff'}
        >
          login
        </a>
        <a 
          href="/docs" 
          style="color: #fff; text-decoration: none; font-family: 'Courier New', monospace; font-size: 14px; hover: color: #ccc;"
          onMouseOver={(e) => e.target.style.color = '#ccc'}
          onMouseOut={(e) => e.target.style.color = '#fff'}
        >
          docs
        </a>
        <a 
          href="https://github.com/monadoid/cubby/" 
          target="_blank" 
          rel="noopener noreferrer"
          style="color: #fff; text-decoration: none; font-family: 'Courier New', monospace; font-size: 14px; hover: color: #ccc;"
          onMouseOver={(e) => e.target.style.color = '#ccc'}
          onMouseOut={(e) => e.target.style.color = '#fff'}
        >
          github
        </a>
      </div>
    </div>
  );
}
