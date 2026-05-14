import type { NextConfig } from "next";

const API_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:3001";

// OSS builds: `NEXT_PUBLIC_KAYA_BUILD=oss pnpm build` emits a static export
// that gets embedded into the kaya-oss binary via rust-embed.
const isOssBuild = process.env.NEXT_PUBLIC_KAYA_BUILD === "oss";

const nextConfig: NextConfig = {
  ...(isOssBuild
    ? {
        output: "export",
        // Static export can't use rewrites; the OSS binary handles CORS directly.
      }
    : {
        async rewrites() {
          return [
            {
              source: "/backend/:path*",
              destination: `${API_URL}/:path*`,
            },
          ];
        },
      }),
};

export default nextConfig;
