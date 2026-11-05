/** Whether a link target is a web or mail address, so no other scheme becomes a link. */
export const linkable = (href?: string | null): href is string => !!href && /^(https?:|mailto:)/i.test(href);

/** Whether a link target is a web address. */
export const isWeb = (href?: string | null): href is string => !!href && /^https?:/i.test(href);

/** The host of a web address without www, or the text as given. */
export const host = (url: string) => {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return url;
  }
};
