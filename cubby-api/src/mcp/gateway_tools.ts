/**
 * Gateway MCP Tools
 *
 * Implements gateway-level tools (devices/list, devices/set)
 * that are handled locally in the API rather than proxied to devices.
 */

import { z } from "zod";
import type { Bindings } from "../index";
import { createDbClient } from "../db/client";
import { getDevicesByUserId, getDeviceForUser } from "../db/devices_repo";
import { initializeDeviceSession } from "./device_client";
import { setDevice as setSessionDevice } from "./session_store";

// Input schemas
const devicesListInputSchema = z.object({});

const devicesSetInputSchema = z.object({
  device_id: z.string().min(1).describe("device id to select"),
});

// Tool definitions for tools/list response
export const GATEWAY_TOOLS = [
  {
    name: "devices/list",
    description: "list all devices owned by the authenticated user",
    inputSchema: z.toJSONSchema(devicesListInputSchema),
  },
  {
    name: "devices/set",
    description:
      "select a device to use for subsequent tool calls. this will establish a session with the device and enable access to device-specific tools.",
    inputSchema: z.toJSONSchema(devicesSetInputSchema),
  },
];

/**
 * Handle devices/list tool call
 */
export async function handleDevicesList(
  env: Bindings,
  userId: string,
): Promise<any> {
  const db = createDbClient(env.DATABASE_URL);
  const devices = await getDevicesByUserId(db, userId);

  return {
    content: [
      {
        type: "text" as const,
        text: `found ${devices.length} device${devices.length === 1 ? "" : "s"}`,
      },
    ],
    structuredContent: {
      devices: devices.map((d) => ({
        id: d.id,
        userId: d.userId,
        createdAt: d.createdAt.toISOString(),
        updatedAt: d.updatedAt.toISOString(),
      })),
    },
  };
}

/**
 * Handle devices/set tool call
 */
export async function handleDevicesSet(
  env: Bindings,
  userId: string,
  gwSessionId: string,
  args: unknown,
): Promise<any> {
  const parsed = devicesSetInputSchema.safeParse(args);
  if (!parsed.success) {
    throw new Error(`invalid arguments: ${parsed.error.message}`);
  }

  const { device_id } = parsed.data;

  // Validate device ID format
  if (!/^[a-zA-Z0-9-]+$/.test(device_id)) {
    throw new Error("invalid device id format");
  }

  // Verify user owns the device
  const db = createDbClient(env.DATABASE_URL);
  const device = await getDeviceForUser(db, device_id, userId);

  if (!device) {
    throw new Error(`device not found or access denied: ${device_id}`);
  }

  // Initialize device MCP session
  console.log(
    `initializing device session for ${device_id} (user: ${userId}, gw session: ${gwSessionId})`,
  );
  const deviceSessionId = await initializeDeviceSession(env, device_id, {
    userId,
    gwSessionId,
  });

  // Store mapping in session store
  setSessionDevice(gwSessionId, device_id, deviceSessionId);

  console.log(
    `device ${device_id} selected for gateway session ${gwSessionId} (device session: ${deviceSessionId})`,
  );

  return {
    content: [
      {
        type: "text" as const,
        text: `device ${device_id} selected. you can now call device-specific tools.`,
      },
    ],
    structuredContent: {
      device_id,
      device_session_id: deviceSessionId,
    },
  };
}

