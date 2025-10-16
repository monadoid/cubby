import { Hono } from 'hono';
import { installScript } from './install.sh.js';

type Bindings = {
	RELEASES: R2Bucket;
};

const app = new Hono<{ Bindings: Bindings }>();

// Serve the install script at /cli endpoint
app.get('/cli', (c) => {
	return c.text(installScript, 200, {
		'Content-Type': 'text/plain',
		'Cache-Control': 'public, max-age=300', // Cache for 5 minutes
	});
});

// Serve binaries from R2 storage
app.get('/binaries/:filename', async (c) => {
	const filename = c.req.param('filename');

	// Get the binary from R2
	const object = await c.env.RELEASES.get(`latest/${filename}`);

	if (!object) {
		return c.text('Binary not found', 404);
	}

	return new Response(object.body, {
		headers: {
			'Content-Type': 'application/octet-stream',
			'Content-Disposition': `attachment; filename="${filename}"`,
			'Cache-Control': 'public, max-age=3600', // Cache for 1 hour
		},
	});
});

// Health check
app.get('/', (c) => {
	return c.json({
		status: 'ok',
		message: 'Cubby distribution server',
		install: 'curl -fsSL https://get.cubby.sh/cli | sh',
	});
});

export default app;
