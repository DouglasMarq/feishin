import { useCallback, useMemo } from 'react';

import { useIsPlayerFetching } from '/@/renderer/features/player/context/player-context';
import { useLongPress } from '/@/shared/hooks/use-long-press';

interface UsePlayButtonClickOptions {
    disabled?: boolean;
    loading?: boolean;
    onClick?: (e: React.MouseEvent<HTMLButtonElement>) => void;
    onLongPress?: (e: React.MouseEvent<HTMLButtonElement>) => void;
}

interface UsePlayButtonClickReturn {
    handlers: {
        onClick: (e: React.MouseEvent<HTMLButtonElement>) => void;
        onMouseDown: (e: React.MouseEvent<HTMLButtonElement>) => void;
        onMouseLeave: (e: React.MouseEvent) => void;
        onMouseUp: (e: React.MouseEvent) => void;
        onTouchCancel: (e: React.TouchEvent) => void;
        onTouchEnd: (e: React.TouchEvent) => void;
        onTouchStart: (e: React.TouchEvent) => void;
    };
    props: {
        disabled: boolean;
        style: React.CSSProperties;
    };
}

export const usePlayButtonClick = ({
    loading,
    onClick,
}: UsePlayButtonClickOptions): UsePlayButtonClickReturn => {
    const isPlayerFetching = useIsPlayerFetching();
    const isDisabled = Boolean(isPlayerFetching || loading);
    const handlePress = (e: React.MouseEvent<HTMLButtonElement> | React.TouchEvent) => {
        if (isDisabled || loading) {
            return;
        }

        e.stopPropagation();
        e.preventDefault();
        onClick?.(e as React.MouseEvent<HTMLButtonElement>);
    };

    const longPressHandlers = useLongPress<HTMLButtonElement>({
        onClick: handlePress,
        onLongPress: handlePress,
    });

    const handleMouseDown = useCallback(
        (e: React.MouseEvent<HTMLButtonElement>) => {
            e.stopPropagation();
            longPressHandlers.onMouseDown?.(e);
        },
        [longPressHandlers],
    );

    const handleClick = useCallback((e: React.MouseEvent<HTMLButtonElement>) => {
        e.stopPropagation();
        e.preventDefault();
    }, []);

    const handlers = useMemo(
        () => ({
            onClick: handleClick,
            onMouseDown: handleMouseDown,
            onMouseLeave: longPressHandlers.onMouseLeave,
            onMouseUp: longPressHandlers.onMouseUp,
            onTouchCancel: longPressHandlers.onTouchCancel,
            onTouchEnd: longPressHandlers.onTouchEnd,
            onTouchStart: longPressHandlers.onTouchStart,
        }),
        [
            handleClick,
            handleMouseDown,
            longPressHandlers.onMouseLeave,
            longPressHandlers.onMouseUp,
            longPressHandlers.onTouchCancel,
            longPressHandlers.onTouchEnd,
            longPressHandlers.onTouchStart,
        ],
    );

    const props = useMemo(() => {
        return {
            disabled: isDisabled,
            style: {
                opacity: isDisabled ? 0.5 : 1,
                transition: 'opacity 0.2s ease-in-out, transform 0.2s ease-in-out',
            } as React.CSSProperties,
        };
    }, [isDisabled]);

    return { handlers, props };
};
