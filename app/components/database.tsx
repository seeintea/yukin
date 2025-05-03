import { useEffect, useRef } from "react";
import { init } from "~/database";

export default function Database() {
  const first = useRef(false);
  useEffect(() => {
    if (first.current) return;
    first.current = true;
    init();
  }, []);
  return null;
}
