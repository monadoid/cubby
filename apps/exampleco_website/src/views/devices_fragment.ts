type Device = {
  id: string
  userId: string
  createdAt: string
  updatedAt: string
}

export function renderDevicesFragment(devices: Device[]): string {
  if (!devices || devices.length === 0) {
    return '<option value="">No devices found</option>'
  }
  
  return devices.map(device => 
    `<option value="${escapeHtml(device.id)}">${escapeHtml(device.id)} (created: ${new Date(device.createdAt).toLocaleDateString()})</option>`
  ).join('\n')
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;')
}

