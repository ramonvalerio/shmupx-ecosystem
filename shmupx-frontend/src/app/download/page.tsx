import type { Metadata } from "next";
import DownloadView from "./DownloadView";

export const metadata: Metadata = {
  title: "ShmupX — Download",
  description: "Baixe o ShmupX, a arena global dos shmups.",
};

export default function DownloadPage() {
  return <DownloadView />;
}
