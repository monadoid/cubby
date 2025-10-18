"use client";

import { createClient, cubbyQueryParams } from "@cubby/js";
import { useState } from "react";

export default function Home() {
  const [loading, setLoading] = useState(false);
  const [results, setResults] = useState<any[]>([]);

  const handleSearch = async () => {
    setLoading(true);
    try {
      const params: cubbyQueryParams = {
        q: "",
        contentType: "ocr",
        limit: 10,
        startTime: new Date(Date.now() - 24 * 60 * 60 * 1000).toISOString(),
      };

      const client = createClient();
      const response = await client.search(params);
      if (response) setResults(response.data);
    } catch (error) {
      console.error("search failed:", error);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen p-8 bg-gray-50 font-mono">
      <div className="max-w-6xl mx-auto">
        <h3 className="text-2xl mb-8 font-bold text-gray-800">cubby search demo</h3>
        <div className="flex gap-4 mb-8">
          <button
            onClick={handleSearch}
            disabled={loading}
            className="px-6 py-2 bg-gray-800 text-white rounded-lg hover:bg-gray-700 disabled:opacity-50"
          >
            {loading ? "searching..." : "search"}
          </button>
        </div>
        <div className="grid grid-cols-2 gap-8">
          <div>
            <h4 className="text-sm font-bold mb-4 text-gray-600">results ({results.length})</h4>
            <div className="space-y-2">
              {results.map((result, i) => (
                <div key={i} className="p-3 bg-white rounded-lg border border-gray-200">
                  <div className="text-sm">
                    <span className="truncate block">{JSON.stringify(result)}</span>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
