import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type UIEvent,
  type WheelEvent,
} from "react";

const BOTTOM_THRESHOLD = 80;

function isNearBottom(element: HTMLDivElement) {
  const distance =
    element.scrollHeight - element.scrollTop - element.clientHeight;

  return distance <= BOTTOM_THRESHOLD;
}

/**
 * 控制聊天消息区域的智能滚动：用户停留在底部时跟随新内容，向上阅读时暂停跟随。
 */
export function useChatScroll(contentVersion: unknown) {
  const containerRef = useRef<HTMLDivElement>(null);
  const shouldFollowRef = useRef(true);
  const isScrollingToBottomRef = useRef(false);
  const [isFollowing, setIsFollowing] = useState(true);

  const updateFollowing = useCallback((following: boolean) => {
    shouldFollowRef.current = following;
    setIsFollowing(following);
  }, []);

  const handleScroll = useCallback(
    (event: UIEvent<HTMLDivElement>) => {
      const nearBottom = isNearBottom(event.currentTarget);

      // 平滑回到底部期间会产生多次 scroll 事件，不能被中间位置误判为用户上滚。
      if (isScrollingToBottomRef.current && !nearBottom) return;

      if (nearBottom) isScrollingToBottomRef.current = false;
      updateFollowing(nearBottom);
    },
    [updateFollowing],
  );

  const handleWheel = useCallback(
    (event: WheelEvent<HTMLDivElement>) => {
      if (event.deltaY >= 0 || event.currentTarget.scrollTop <= 0) return;

      isScrollingToBottomRef.current = false;
      updateFollowing(false);
    },
    [updateFollowing],
  );

  const handlePointerDown = useCallback(() => {
    // 用户开始拖动滚动条或触摸消息区时，后续 scroll 事件应按真实位置重新判断。
    isScrollingToBottomRef.current = false;
  }, []);

  const scrollToBottom = useCallback(() => {
    const container = containerRef.current;
    if (!container) return;

    isScrollingToBottomRef.current = true;
    updateFollowing(true);
    container.scrollTo({ top: container.scrollHeight, behavior: "smooth" });
  }, [updateFollowing]);

  useEffect(() => {
    if (!shouldFollowRef.current) return;

    // 等 React 提交新的消息高度后，在下一帧贴到容器底部。
    const frame = requestAnimationFrame(() => {
      const container = containerRef.current;
      if (container) container.scrollTop = container.scrollHeight;
    });

    return () => cancelAnimationFrame(frame);
  }, [contentVersion]);

  return {
    containerRef,
    isFollowing,
    scrollToBottom,
    handleScroll,
    handleWheel,
    handlePointerDown,
  };
}
