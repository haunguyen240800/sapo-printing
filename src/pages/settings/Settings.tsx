import { BlockStack, Box, Checkbox, Text } from "@sapo/ui-components";
import { useEffect, useState } from "react";
import { getAutostartEnabled, setAutostartEnabled } from "src/services/printer-service";

export default function SettingsPage() {
  const [autostart, setAutostart] = useState(false);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    getAutostartEnabled()
      .then(setAutostart)
      .catch(() => setAutostart(false))
      .finally(() => setLoading(false));
  }, []);

  const handleToggle = async (checked: boolean) => {
    const previous = autostart;
    setAutostart(checked);
    try {
      await setAutostartEnabled(checked);
    } catch {
      setAutostart(previous);
    }
  };

  return (
    <Box padding="4">
      <BlockStack gap="4">
        <Text as="h2" variant="headingMd">
          Cài đặt chung
        </Text>
        <Checkbox
          checked={autostart}
          disabled={loading}
          onChange={handleToggle}
          label="Khởi động cùng Windows"
          helpText="Tự động chạy ngầm khi bật máy. Đóng cửa sổ sẽ thu nhỏ ứng dụng xuống khay hệ thống thay vì thoát."
        />
      </BlockStack>
    </Box>
  );
}
