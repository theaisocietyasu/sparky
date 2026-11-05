import { liveResult } from "@/components/discord/format";
import { LIVE } from "@/components/discord/examples";
import type { Turn } from "./turn";

/** Morning: what is due this week, from Canvas. */
export const DUE: Turn = {
  question: "morning. what's due this week, and did any of my profs post something I should know?",
  thought: "Two Canvas reads: the assignments due this week and recent announcements across courses.",
  calls: [
    {
      tool: "canvas_assignments",
      args: {},
      output:
        "4 upcoming assignments: CSE 340 Project 2, due Oct 7 11:59pm. MAT 343 Homework 6, due Oct 8 11:59pm. CSE 310 Quiz 4, due Oct 9 5:00pm. ENG 302 Draft 2, due Oct 11 11:59pm.",
    },
    {
      tool: "canvas_announcements",
      args: {},
      output:
        "3 recent announcements: CSE 310, Quiz 4 moves from Wednesday to Friday. MAT 343, no office hours this Thursday. ENG 302, peer review groups are posted.",
    },
  ],
  wrap: "Order the deadlines and flag the announcements that change this week.",
  answer:
    "Four things are due this week:\n- **CSE 340** Project 2, Wednesday 11:59 PM\n- **MAT 343** Homework 6, Thursday 11:59 PM\n- **CSE 310** Quiz 4, now **Friday** 5 PM (moved from Wednesday)\n- **ENG 302** Draft 2, Sunday 11:59 PM\n\nMAT 343 has no office hours this Thursday, so ask your Homework 6 questions before then.",
};

/** Mid morning: what in the inbox needs action today, from Outlook. */
export const INBOX: Turn = {
  question: "anything in my inbox I actually need to deal with today?",
  thought: "Read recent Outlook mail and today's calendar, then keep only what needs action today.",
  calls: [
    {
      tool: "outlook_mail",
      args: {},
      output:
        "10 recent messages: Please confirm your 2:00pm appointment (Academic Advising). Re: Project 1 regrade (CSE 340 TA). Career fair registration closes Oct 9 (Fulton Career Center). Your statement is ready (Sun Card Office). This week at the MU (Memorial Union).",
    },
    { tool: "outlook_calendar", args: {}, output: "2 events today: CSE 340 lecture 10:30am, BYENG 210. Advising appointment 2:00pm, Student Services." },
  ],
  wrap: "Three messages need a reply or a step; the rest are newsletters.",
  answer:
    "Three need you today:\n- **Advising** wants you to confirm your **2 PM** appointment. It is on your calendar.\n- The **CSE 340 TA** answered your Project 1 regrade request.\n- **Career fair** registration closes Friday.\n\nThe rest is newsletters and a Sun Card statement. Nothing to do there.",
};

/** Midday: food and a place to study, both live, asked in the server. */
export const BETWEEN: Turn = {
  question: "got 90 min before my 2pm. where can I eat near the MU, and where can I study after?",
  thought: "Two live reads at once: dining open now at the Memorial Union and library hours today.",
  calls: [
    {
      tool: "search",
      args: { query: "Memorial Union dining open now", source: LIVE.dining.key },
      output: liveResult(LIVE.dining.key, LIVE.dining.url, "Memorial Union. Open now: Pitchforks, Panda Express, Chick-fil-A, until 3:00pm."),
    },
    {
      tool: "search",
      args: { query: "library hours today", source: LIVE.libraryHours.key },
      output: liveResult(LIVE.libraryHours.key, LIVE.libraryHours.url, "Hayden Library 7:00am to 12:00am. Noble Library 7:00am to 10:00pm."),
    },
  ],
  wrap: "Food, then a library, in the order of the afternoon.",
  answer:
    "Eat at the **Memorial Union** first: Pitchforks, Panda Express and Chick-fil-A are open until 3 PM.\n\nThen study at **Hayden Library**, open until midnight, a few minutes' walk away.",
};

/** Afternoon: book a group study room, held for approval. */
export const BOOK: Turn = {
  question: "book a study room at Hayden for 4 people tomorrow 6 to 8pm, for my CSE 310 group",
  thought: "Find a Hayden group room free tomorrow from 6 to 8 PM, then book it. Booking needs approval.",
  calls: [
    {
      tool: "search",
      args: { query: "Hayden group room 4 people Oct 7 6pm to 8pm", source: LIVE.studyRooms.key },
      output: liveResult(LIVE.studyRooms.key, LIVE.studyRooms.url, "Hayden Library group study rooms, capacity 4. Free Oct 7, 6:00pm to 8:00pm: C-21, C-24."),
    },
  ],
  wrap: "C-21 is free for the whole slot. Book it once approved.",
  answer: "Room **C-21** in Hayden Library is free tomorrow from 6 to 8 PM.",
  held: {
    tool: "book_study_room",
    args: '{"date":"2026-10-07","end":"20:00","people":4,"room":"Hayden C-21","start":"18:00"}',
    done: "Booked **Hayden C-21** for tomorrow, 6 to 8 PM. The library sent the confirmation to your ASU email.",
  },
};

/** Evening: the score the final needs, from Canvas grades and a computation. */
export const GRADE: Turn = {
  question: "what do I need on the CSE 340 final to keep an A?",
  thought: "Read the CSE 340 scores and weights from Canvas, then compute the final score that reaches 90 percent.",
  calls: [
    {
      tool: "canvas_assignment_grades",
      args: {},
      output: "CSE 340. Projects 30 percent: Project 1 88/100. Homework 20 percent: average 97. Midterm 20 percent: 91/100. Final 30 percent: not graded.",
    },
    { tool: "run_sandbox", args: { command: 'python3 -c "print((90 - (0.3*88 + 0.2*97 + 0.2*91)) / 0.3)"' }, output: "86.66666666666667" },
  ],
  wrap: "About 87 percent, if Project 2 lands near Project 1.",
  answer:
    "You need about **87%** on the final to finish CSE 340 with an A (90%).\n\nThat assumes Project 2 lands near your Project 1 score of 88. Every point above that on Project 2 lowers what the final needs.",
};

/** Night: the week in one list, from three calendars. */
export const WEEK: Turn = {
  question: "put my week in one list: deadlines, classes stuff, and my library shifts",
  thought: "Merge the Canvas calendar, Outlook calendar and Google Calendar into one list by day.",
  calls: [
    { tool: "canvas_calendar", args: {}, output: "4 upcoming events: CSE 340 Project 2 due Oct 7. MAT 343 Homework 6 due Oct 8. CSE 310 Quiz 4 Oct 9. ENG 302 Draft 2 due Oct 11." },
    { tool: "outlook_calendar", args: {}, output: "2 upcoming events: CSE 310 group study Oct 7 6:00pm, Hayden C-21. Career fair registration closes Oct 9." },
    { tool: "google_calendar", args: {}, output: "2 upcoming events: Library shift Oct 7 4:00pm. Library shift Oct 9 4:00pm." },
  ],
  wrap: "Group the events by day, earliest first.",
  answer:
    "**Wednesday**\n- 4 PM library shift\n- 6 PM CSE 310 group, Hayden C-21\n- 11:59 PM CSE 340 Project 2\n**Thursday**\n- 11:59 PM MAT 343 Homework 6\n**Friday**\n- 4 PM library shift\n- 5 PM CSE 310 Quiz 4\n- Career fair registration closes\n**Sunday**\n- 11:59 PM ENG 302 Draft 2",
};
