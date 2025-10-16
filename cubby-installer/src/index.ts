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

// Get the latest version by listing versioned directories
app.get('/version/latest', async (c) => {
	try {
		// List all objects to find version directories
		const listed = await c.env.RELEASES.list({ delimiter: '/' });
		
		// Extract version numbers from prefixes (e.g., "0.1.5/")
		const versions = listed.delimitedPrefixes
			.map(prefix => prefix.replace('/', ''))
			.filter(v => v !== 'latest' && /^\d+\.\d+\.\d+$/.test(v))
			.sort((a, b) => {
				const [aMajor, aMinor, aPatch] = a.split('.').map(Number);
				const [bMajor, bMinor, bPatch] = b.split('.').map(Number);
				if (aMajor !== bMajor) return bMajor - aMajor;
				if (aMinor !== bMinor) return bMinor - aMinor;
				return bPatch - aPatch;
			});

		const latestVersion = versions[0];
		
		if (!latestVersion) {
			return c.text('No versions found', 404);
		}

		return c.text(latestVersion, 200, {
			'Content-Type': 'text/plain',
			'Cache-Control': 'public, max-age=300', // Cache for 5 minutes
		});
	} catch (error) {
		console.error('Error fetching latest version:', error);
		return c.text('Error fetching version', 500);
	}
});

// Serve binaries from R2 storage (supports both latest/ and versioned paths)
app.get('/binaries/:filename', async (c) => {
	const filename = c.req.param('filename');

	// Get the binary from R2 (try latest/ first for backwards compatibility)
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
