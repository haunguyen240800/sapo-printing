import { Modal, Select2 } from '@sapo/ui-components';
import { useState } from 'react';

interface ConnectionConfigModalProps {
    open: boolean;
    onClose: () => void;
}

const ConnectionConfigModal = ({ open, onClose }: ConnectionConfigModalProps) => {
    const [endpoint, setEndpoint] = useState('sapo-api');

    const handleSave = () => {
        // Handle save logic
        onClose();
    };

    const handleTest = () => {
        // Handle test connection logic
    };

    return (
        <Modal
            open={open}
            onClose={onClose}
            title="Chỉnh sửa cấu hình"
            size="small"
            sectioned
            primaryAction={{
                content: 'Lưu cấu hình',
                onAction: handleSave,
            }}
            secondaryActions={[
                {
                    content: 'Hủy',
                    outline: true,
                    onAction: onClose,
                },
                {
                    content: 'Kiểm tra kết nối',
                    outline: true,
                    onAction: handleTest,
                }
            ]}
        >
            <Select2
                label=""
                options={[
                    { label: 'https://www.sapo.vn/api/v1/print/plugin', value: 'sapo-api' }
                ]}
                value={endpoint}
                onChange={(val) => setEndpoint(val)}
            />
        </Modal>
    );
};

export default ConnectionConfigModal;
