// Declare global interface
window.canvasEl = null;
window.renderEl = null;

console.log('Fluid component script starting...');

// Wait for fonts to load and initialize
document.fonts.ready.then(() => {
  console.log('Fonts ready, initializing fluid effect...');
  
  // Get canvas and render elements
  const canvas = document.getElementById('canvas');
  const renderDiv = document.getElementById('render');
  
  console.log('Canvas found:', !!canvas);
  console.log('Render div found:', !!renderDiv);
  
  if (canvas && renderDiv) {
    // Set canvas size to full viewport
    canvas.width = window.innerWidth;
    canvas.height = window.innerHeight;
    
    console.log('Canvas size set:', canvas.width, 'x', canvas.height);
    
    // Expose them on window so triangle_script.js can find them
    window.canvasEl = canvas;
    window.renderEl = renderDiv;

    console.log('Window objects set:', !!window.canvasEl, !!window.renderEl);

    // Dynamically load and attach the script
    const script = document.createElement('script');
    script.src = '/triangle_script.js';
    script.async = true;
    
    script.onload = () => {
      console.log('triangle_script.js loaded successfully');
    };
    
    script.onerror = (error) => {
      console.error('Failed to load triangle_script.js:', error);
    };
    
    document.body.appendChild(script);
    console.log('Script element added to DOM');
  } else {
    console.error('Canvas or render div not found');
  }
});
