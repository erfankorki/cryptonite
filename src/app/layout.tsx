import type { Metadata } from "next";
import { Geist_Mono } from "next/font/google";
import { ThemeProvider } from "next-themes";
import QueryProvider from "@/providers/query-provider";
import "./globals.css";
import type { ReactNode } from "react";

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

export const metadata: Metadata = {
  title: "Cryptonite",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: ReactNode;
}>) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body className={`${geistMono.variable}`}>
        <QueryProvider>
          <ThemeProvider>
            {children}
          </ThemeProvider>

        </QueryProvider>
      </body>
    </html>
  );
}
