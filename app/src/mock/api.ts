import type { WrasseApi } from "../api";
import { generateDisk } from "./disk";

const isWindows = typeof navigator !== "undefined" && navigator.userAgent.includes("Windows");

export const mockApi: WrasseApi = {
  async appInfo() {
    return {
      version: "0.1.0",
      platform: isWindows ? "Windows" : "macOS",
      fileManager: isWindows ? "File Explorer" : "Finder",
      units: isWindows ? "binary" : "decimal",
      dev: true,
      bench: false,
    };
  },
  async scan({ files = 3500, uniform = false } = {}) {
    return generateDisk(files, uniform);
  },
};
