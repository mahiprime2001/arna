import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Arna — Remote control for your store network",
  description:
    "Arna is a self-hosted, end-to-end encrypted platform for remote support, fleet monitoring, chat, meetings, and file sharing across your stores.",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body className="font-sans antialiased">{children}</body>
    </html>
  );
}
