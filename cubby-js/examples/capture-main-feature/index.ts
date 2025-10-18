import { createClient } from "@cubby/js";

async function start() {
  console.log("sending demo notifications via gateway...");
  const client = createClient();
  await client.notify({ title: "less useful feature", body: "dog: woof" } as any);
  await client.notify({ title: "very useful feature", body: "cat: meow" } as any);
}

start().catch(console.error);
