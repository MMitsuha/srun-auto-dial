"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

const links = [
  { href: "/", label: "Dashboard" },
  { href: "/login", label: "Login" },
  { href: "/random", label: "Random" },
];

export function Navbar() {
  const pathname = usePathname();

  return (
    <nav className="border-b border-neutral-800 bg-black/80 backdrop-blur-md sticky top-0 z-50">
      <div className="mx-auto flex h-14 max-w-3xl items-center gap-6 px-6">
        <Link
          href="/"
          className="font-semibold tracking-tight text-white mr-4"
        >
          Srun Auto Dial
        </Link>
        <div className="flex gap-1">
          {links.map(({ href, label }) => (
            <Link
              key={href}
              href={href}
              className={`rounded-md px-3 py-1.5 text-sm transition-colors ${
                pathname === href
                  ? "bg-neutral-800 text-white"
                  : "text-neutral-400 hover:text-white"
              }`}
            >
              {label}
            </Link>
          ))}
        </div>
      </div>
    </nav>
  );
}
