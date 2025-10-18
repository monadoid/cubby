"use client";

import Image from "next/image";
import { useEffect, useState } from "react";
import { createClient } from "@cubby/js";
import type { VisionEvent } from "@cubby/js";

export default function Home() {
  const [visionEvent, setVisionEvent] = useState<VisionEvent | null>(null);

  useEffect(() => {
    const streamVision = async () => {
      try {
        const client = createClient({ 
          env: { 
            CUBBY_API_BASE_URL: process.env.NEXT_PUBLIC_CUBBY_API_BASE_URL || "https://api.cubby.sh" 
          },
          clientId: process.env.NEXT_PUBLIC_CUBBY_CLIENT_ID,
          clientSecret: process.env.NEXT_PUBLIC_CUBBY_CLIENT_SECRET,
        });
        for await (const event of client.streamVision(true)) {
          setVisionEvent(event.data);
          console.log("vision event received");
        }
      } catch (error) {
        console.error("vision stream error:", error);
      }
    };

    streamVision();

    return () => {
      // no persistent connection object; the generator closes the socket on exit
    };
  }, []);

  return (
    <div className="min-h-screen flex flex-col items-center justify-center bg-transparent gap-4">
      {visionEvent?.image ? (
        <div className="space-y-4">
          <Image
            src={`data:image/jpeg;base64,${visionEvent.image}`}
            alt="streamed content"
            width={500}
            height={300}
            style={{ objectFit: "contain" }}
            className="rounded-lg"
          />
          <div className="space-y-2 font-mono text-sm">
            <a href={visionEvent.browser_url} target="_blank">
              <p className="text-gray-500">
                {visionEvent.browser_url || "unknown"}
              </p>
            </a>
            <p className="text-gray-500">
              app: {visionEvent.app_name || "unknown"}
            </p>
            <p className="text-gray-500">
              window: {visionEvent.window_name || "unknown"}
            </p>
            <p className="text-gray-500">
              time: {new Date(visionEvent.timestamp).toLocaleTimeString()}
            </p>
            {visionEvent.text && (
              <p className="text-gray-500 max-w-[500px] break-words">
                text: {visionEvent.text}
              </p>
            )}
          </div>
        </div>
      ) : (
        <div className="animate-pulse bg-gray-200 rounded-lg w-[500px] h-[300px]" />
      )}

      <div className="fixed bottom-4 right-4 text-sm text-gray-500 font-mono">
        {visionEvent ? "streaming..." : "waiting for stream..."}
      </div>
    </div>
  );
}
