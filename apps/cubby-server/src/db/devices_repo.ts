import { z } from 'zod';
import { createInsertSchema } from 'drizzle-zod';
import { and, eq } from 'drizzle-orm';

import type { DbClient } from './client';
import { devices } from './schema';

const baseDeviceInsertSchema = createInsertSchema(devices, {
    userId: () => z.string().uuid('Invalid user id'),
});

export const createDeviceSchema = baseDeviceInsertSchema.pick({
    userId: true,
});

export type CreateDeviceInput = z.infer<typeof createDeviceSchema>;

export async function createDevice(db: DbClient, input: CreateDeviceInput) {
    const [device] = await db
        .insert(devices)
        .values({ userId: input.userId })
        .returning();
    return device;
}

export async function getDevicesByUserId(db: DbClient, userId: string) {
    return db
        .select()
        .from(devices)
        .where(eq(devices.userId, userId));
}

export async function getDeviceForUser(db: DbClient, deviceId: string, userId: string) {
    const [device] = await db
        .select()
        .from(devices)
        .where(
            and(eq(devices.id, deviceId), eq(devices.userId, userId))
        )
        .limit(1);

    return device;
}
