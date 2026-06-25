export const PaperSizeOptions = [
  {label: 'A4 (210 × 297 mm)', value: 'A4'},
  {label: 'A5 (148 × 210 mm)', value: 'A5'},
  {label: 'Letter (216 × 279 mm)', value: 'Letter'},
  {label: '10cm × 10cm', value: 'CM10x10'},
  {label: '10cm × 12cm', value: 'CM10x12'},
  {label: '10cm × 15cm', value: 'CM10x15'},
  {label: '10cm × 18cm', value: 'CM10x18'},
  {label: 'Tùy chỉnh', value: 'Custom'},
] as const;

export const ImageFormatOptions = [
  {label: 'RGB', value: 'RGB'},
  {label: 'ARGB', value: 'ARGB'},
  {label: 'BGR', value: 'BGR'},
  {label: 'GRAY', value: 'GRAY'},
  {label: 'BINARY', value: 'BINARY'},
] as const;

export const DefaultPrintConfig = {
  printerName: '',
  paperSize: 'A4',
  width: 210,
  height: 297,
  marginLeft: 0,
  marginRight: 0,
  marginTop: 0,
  marginBottom: 0,
  landscape: false,
  printingBuffer: false,
  printImage: false,
  imageFormat: 'RGB',
} as const;

const PAPER_DIMENSIONS: Record<string, {width: string; height: string}> = {
  A4: {width: '210', height: '297'},
  A5: {width: '148', height: '210'},
  Letter: {width: '216', height: '279'},
  CM10x10: {width: '100', height: '100'},
  CM10x12: {width: '100', height: '120'},
  CM10x15: {width: '100', height: '150'},
  CM10x18: {width: '100', height: '180'},
};

export function getDimensionsByPaperSize(size: string): {width: string; height: string} {
  return PAPER_DIMENSIONS[size];
}