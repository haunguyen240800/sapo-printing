import { ActionList, ActionListProps, Button, ButtonProps, Popover } from "@sapo/ui-components";

import { useToggle } from "../../utils/useToggle";

type Props = ButtonProps & {
  actions: ActionListProps["items"];
};

export const ActionListButton = ({ actions, children, ...props }: Props) => {
  const { value: isOpen, toggle: togglePopover, setFalse: closePopover } = useToggle(false);

  return (
    <Popover
      activator={
        <Button {...props} onClick={togglePopover} disclosure={isOpen ? "up" : "down"}>
          {children}
        </Button>
      }
      active={isOpen}
      onClose={closePopover}
    >
      <ActionList items={actions} onActionAnyItem={closePopover} />
    </Popover>
  );
};
