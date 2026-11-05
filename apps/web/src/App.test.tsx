import { render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import App from "./App";

const REPO = "https://github.com/theaisocietyasu/sparky";

describe("website routes", () => {
  beforeEach(() => {
    window.history.pushState(null, "", "/");
  });

  it("leads with what Sparky is and how to run it", () => {
    render(<App />);

    const main = within(screen.getByRole("main"));
    expect(main.getByRole("heading", { level: 1 })).toHaveTextContent(/your university\s*copilot/i);
    expect(main.getByRole("link", { name: /chat with sparky/i })).toHaveAttribute("href", "/chat");
    expect(main.getByRole("link", { name: /run your own/i })).toHaveAttribute("href", `${REPO}#run-it`);
  });

  it("shows every section of the product", () => {
    render(<App />);

    for (const label of [/sparky in action/i, /web chat/i, /what students ask/i, /how it works/i, /open source/i]) {
      expect(screen.getByRole("region", { name: label })).toBeInTheDocument();
    }
  });

  it("plays the recorded Discord demo in the in action section", () => {
    const { container } = render(<App />);

    const action = screen.getByRole("region", { name: /sparky in action/i });
    expect(within(action).getByLabelText(/student's day with sparky/i)).toHaveAttribute("src", "/demo/sparky-demo.mp4");
    expect(container.querySelector("video")?.getAttribute("poster")).toBe("/demo/sparky-demo.webp");
  });

  it("plays the setup walkthrough in the open source section", () => {
    render(<App />);

    const open = within(screen.getByRole("region", { name: /open source/i }));
    expect(open.getByLabelText(/developer cloning sparkyai/i)).toHaveAttribute("src", "/demo/sparky-setup.mp4");
  });

  it("shows each source as a Discord thread with the steps and a link to the page", () => {
    render(<App />);

    const sources = within(screen.getByRole("region", { name: /what students ask/i }));
    expect(sources.getByRole("link", { name: /asu library hours/i })).toHaveAttribute("href", "https://lib.asu.edu/hours");
    expect(sources.getAllByText(/live result from/i).length).toBeGreaterThan(0);
  });

  it("links the navigation to each section and the repository", () => {
    render(<App />);

    const nav = within(screen.getByRole("navigation", { name: /primary/i }));
    for (const id of ["in-action", "web-chat", "sources", "how-it-works", "open-source"]) {
      expect(document.getElementById(id)).not.toBeNull();
      expect(nav.getByRole("link", { name: new RegExp(id.replace(/-/g, " "), "i") })).toHaveAttribute(
        "href",
        `#${id}`,
      );
    }
    expect(nav.getByRole("link", { name: /github/i })).toHaveAttribute("href", REPO);
    expect(nav.getByRole("link", { name: /open chat/i })).toHaveAttribute("href", "/chat");
  });

  it("serves a not found page for unknown paths, including the retired legacy site", () => {
    window.history.pushState(null, "", "/old");

    render(<App />);

    expect(screen.getByRole("heading", { name: /page not found/i })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /back to home/i })).toHaveAttribute("href", "/");
  });
});
