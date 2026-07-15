export function normalizeWebUrl(value: string): string | null {
  const candidate = value.trim();

  if (!candidate) {
    return null;
  }

  try {
    const url = new URL(candidate);
    if (url.protocol !== "http:" && url.protocol !== "https:") {
      return null;
    }

    return url.toString();
  } catch {
    return null;
  }
}

export function isYoutubeUrl(value: string): boolean {
  const normalized = normalizeWebUrl(value);
  if (!normalized) return false;
  const host = new URL(normalized).hostname.toLowerCase();
  return (
    host === "youtu.be" ||
    host === "youtube.com" ||
    host.endsWith(".youtube.com") ||
    host === "youtube-nocookie.com" ||
    host.endsWith(".youtube-nocookie.com")
  );
}
