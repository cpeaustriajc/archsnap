export const SCOPE = "@earnest";

export const SUPPORTED = [
  "linux-x64-gnu",
  "linux-arm64-gnu",
  "darwin-arm64",
  "darwin-x64",
  "win32-x64-msvc",
] as const;

export type Libc = "gnu" | "musl";

export interface Machine {
  readonly platform: string;
  readonly arch: string;
  readonly libc: Libc | null;
}

export const platformKey = ({ platform, arch, libc }: Machine): string => {
  if (platform === "linux") {
    return `linux-${arch}-${libc ?? "gnu"}`;
  }
  if (platform === "win32") {
    return `win32-${arch}-msvc`;
  }
  return `${platform}-${arch}`;
};

export const packageName = (key: string): string => `${SCOPE}/cli-${key}`;

export const isSupported = (key: string): boolean => SUPPORTED.some((each) => each === key);

export const binaryFile = (platform: string): string =>
  platform === "win32" ? "earnest.exe" : "earnest";

const glibcVersion = (report: unknown): unknown => {
  if (typeof report !== "object" || report === null || !("header" in report)) {
    return undefined;
  }
  const { header } = report;
  if (typeof header !== "object" || header === null || !("glibcVersionRuntime" in header)) {
    return undefined;
  }
  return header.glibcVersionRuntime;
};

const detectLibc = (): Libc => {
  const report: unknown = process.report?.getReport();
  return typeof glibcVersion(report) === "string" ? "gnu" : "musl";
};

export const currentMachine = (): Machine => ({
  platform: process.platform,
  arch: process.arch,
  libc: process.platform === "linux" ? detectLibc() : null,
});
