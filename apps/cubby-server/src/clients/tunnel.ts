const HEALTH_PATH = '/health'

export type TunnelEnv = {
  ACCESS_CLIENT_ID: string
  ACCESS_CLIENT_SECRET: string
  TUNNEL_DOMAIN: string
}

const DEVICE_ID_PATTERN = /^[a-zA-Z0-9-]+$/

function buildTargetUrl(deviceId: string, domain: string) {
  if (!DEVICE_ID_PATTERN.test(deviceId)) {
    throw new Error('Invalid device identifier')
  }

  return `https://${deviceId}.${domain}${HEALTH_PATH}`
}

export async function fetchDeviceHealth(deviceId: string, env: TunnelEnv, userId?: string) {
  const target = buildTargetUrl(deviceId, env.TUNNEL_DOMAIN)
  const requestId = crypto.randomUUID()

  const resp = await fetch(target, {
    method: 'GET',
    headers: {
      'CF-Access-Client-Id': env.ACCESS_CLIENT_ID,
      'CF-Access-Client-Secret': env.ACCESS_CLIENT_SECRET,
      'X-Cubby-Request-Id': requestId,
      ...(userId ? { 'X-Cubby-User': userId } : {}),
    },
  })

  const headers = new Headers(resp.headers)
  headers.set('X-Cubby-Request-Id', requestId)

  return new Response(resp.body, {
    status: resp.status,
    statusText: resp.statusText,
    headers,
  })
}

