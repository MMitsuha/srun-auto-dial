"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

const links = [
  { href: "/", label: "Dashboard" },
  { href: "/login", label: "Connect" },
  { href: "/logout", label: "Disconnect" },
];

export function Navbar() {
  const pathname = usePathname();

  return (
    <nav
      aria-label="Primary navigation"
      className="sticky top-0 z-50 border-b border-white/10 bg-black/70 backdrop-blur-xl"
    >
      <div className="mx-auto flex max-w-5xl flex-col gap-2 px-4 py-3 sm:h-16 sm:flex-row sm:items-center sm:justify-between sm:px-6 sm:py-0">
        <Link
          href="/"
          className="group inline-flex min-h-10 w-fit items-center gap-2 rounded-lg pr-2 font-semibold tracking-tight text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sky-300"
        >
          <span
            aria-hidden="true"
            className="grid h-8 w-8 place-items-center rounded-lg border border-sky-300/30 bg-sky-300/10 text-sm text-sky-300 shadow-[0_0_24px_-8px_rgba(125,211,252,0.9)]"
          >
            S
          </span>
          <span>Srun Control</span>
        </Link>
        <div className="grid grid-cols-3 gap-1" aria-label="Sections">
          {links.map(({ href, label }) => {
            const active = pathname === href;
            return (
              <Link
                key={href}
                href={href}
                aria-current={active ? "page" : undefined}
                className={`inline-flex min-h-11 items-center justify-center rounded-lg px-2.5 text-center text-sm font-medium transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sky-300 sm:px-3.5 ${
                  active
                    ? "bg-white/10 text-white"
                    : "text-neutral-400 hover:bg-white/[0.05] hover:text-white"
                }`}
              >
                {label}
              </Link>
            );
          })}
        </div>
      </div>
    </nav>
  );
}
