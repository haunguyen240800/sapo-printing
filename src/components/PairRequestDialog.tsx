import React, { useEffect, useState } from 'react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { Modal } from '@sapo/ui-components';

interface PairRequestPayload {
  request_id: string;
  origin: string;
}

export const PairRequestDialog: React.FC = () => {
  const [request, setRequest] = useState<PairRequestPayload | null>(null);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;

    const setupListener = async () => {
      unlisten = await listen<PairRequestPayload>('agent-pair-request', (event) => {
        console.log('Pair request received:', event.payload);
        setRequest(event.payload);
      });
    };

    setupListener();

    return () => {
      unlisten?.();
    };
  }, []);

  const handleResolve = async (allow: boolean) => {
    if (!request) return;
    try {
      await invoke('resolve_pair', {
        requestId: request.request_id,
        approved: allow
      });
    } catch (err) {
      console.error('Failed to resolve pair request', err);
    } finally {
      setRequest(null);
    }
  };

  return (
    <Modal
      open={!!request}
      title="Yêu cầu kết nối máy in"
      size="small"
      onClose={() => handleResolve(false)}
      primaryAction={{
        content: 'Đồng ý',
        onAction: () => handleResolve(true)
      }}
      secondaryActions={[
        {
          content: 'Từ chối',
          onAction: () => handleResolve(false)
        }
      ]}
    >
      <Modal.Section>
        <p style={{ margin: 0, lineHeight: 1.5 }}>
          Trang web <strong style={{ color: '#0088FF' }}>{request?.origin}</strong> đang muốn lấy mã Token để ra lệnh in xuống ứng dụng của bạn. Bạn có cho phép không?
        </p>
      </Modal.Section>
    </Modal>
  );
};
