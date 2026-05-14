import { AppNav } from "@/components/shared/AppNav";

export default function SharedLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex h-screen overflow-hidden">
      <AppNav />
      <div className="flex-1 min-w-0 overflow-hidden">{children}</div>
    </div>
  );
}
