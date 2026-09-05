/** Return a normalized public HTTPS URL, or null for unsafe/invalid input. */
export function safeExternalUrl(value: string | null | undefined): string | null {
  const candidate = value?.trim();
  if (!candidate) return null;

  try {
    const url = new URL(candidate);
    if (url.protocol !== "https:" || url.username || url.password) return null;
    return url.href;
  } catch {
    return null;
  }
}

export function hostnameMatches(hostname: string, domain: string): boolean {
  const host = hostname.toLowerCase().replace(/\.$/, "");
  const expected = domain.toLowerCase();
  return host === expected || host.endsWith(`.${expected}`);
}
