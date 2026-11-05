import DragonTrail from "@/components/dragon/DragonTrail";
import Crossing from "@/components/site/Crossing";
import Footer from "@/components/site/Footer";
import Hero from "@/components/site/Hero";
import HowItWorks from "@/components/site/HowItWorks";
import InAction from "@/components/site/InAction";
import Nav from "@/components/site/Nav";
import OnTheWeb from "@/components/site/OnTheWeb";
import OpenSource from "@/components/site/OpenSource";
import Sources from "@/components/site/Sources";
import { useSmoothScroll } from "@/components/site/motion";

/** The landing page, with the dragon winding behind every section. */
const Home = () => {
  useSmoothScroll();
  return (
    <div className="min-h-screen">
      <Nav />
      <main className="relative">
        <DragonTrail />
        <Hero />
        <Crossing />
        <InAction />
        <OnTheWeb />
        <Crossing flip />
        <Sources />
        <Crossing />
        <HowItWorks />
        <Crossing flip />
        <OpenSource />
      </main>
      <Footer />
    </div>
  );
};

export default Home;
