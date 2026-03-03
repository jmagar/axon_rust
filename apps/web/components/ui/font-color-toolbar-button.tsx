'use client'

/* eslint-disable react-hooks/refs -- Ref usage for color picker component refs */

import { useComposedRef } from '@udecode/cn'
import { EraserIcon, PlusIcon } from 'lucide-react'
import { useEditorRef, useEditorSelector } from 'platejs/react'
import React from 'react'

import { buttonVariants } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { debounce } from '@/lib/debounce'
import { cn } from '@/lib/utils'

import { DEFAULT_COLORS, DEFAULT_CUSTOM_COLORS, type TColor } from './font-color-presets'
import { ToolbarButton, ToolbarMenuGroup } from './toolbar'

export interface FontColorToolbarButtonProps extends React.ComponentProps<typeof DropdownMenu> {
  children?: React.ReactNode
  nodeType: string
  tooltip?: string
}

export function FontColorToolbarButton({
  children,
  nodeType,
  tooltip,
}: FontColorToolbarButtonProps) {
  const editor = useEditorRef()

  const selectionDefined = useEditorSelector((currentEditor) => !!currentEditor.selection, [])

  const color = useEditorSelector(
    (currentEditor) => currentEditor.api.mark(nodeType) as string,
    [nodeType],
  )

  const [selectedColor, setSelectedColor] = React.useState<string>()
  const [open, setOpen] = React.useState(false)

  const onToggle = React.useCallback(
    (value = !open) => {
      setOpen(value)
    },
    [open],
  )

  const updateColor = React.useCallback(
    (value: string) => {
      if (editor.selection) {
        setSelectedColor(value)

        editor.tf.select(editor.selection)
        editor.tf.focus()

        editor.tf.addMarks({ [nodeType]: value })
      }
    },
    [editor, nodeType],
  )

  const updateColorAndClose = React.useCallback(
    (value: string) => {
      updateColor(value)
      onToggle()
    },
    [onToggle, updateColor],
  )

  const clearColor = React.useCallback(() => {
    if (editor.selection) {
      editor.tf.select(editor.selection)
      editor.tf.focus()

      if (selectedColor) {
        editor.tf.removeMarks(nodeType)
      }

      onToggle()
    }
  }, [editor, selectedColor, onToggle, nodeType])

  React.useEffect(() => {
    if (selectionDefined) {
      setSelectedColor(color)
    }
  }, [color, selectionDefined])

  return (
    <DropdownMenu
      open={open}
      onOpenChange={(value) => {
        setOpen(value)
      }}
      modal={false}
    >
      <DropdownMenuTrigger asChild>
        <ToolbarButton pressed={open} tooltip={tooltip}>
          {children}
        </ToolbarButton>
      </DropdownMenuTrigger>

      <DropdownMenuContent align="start">
        <ColorPicker
          color={selectedColor || color}
          clearColor={clearColor}
          colors={DEFAULT_COLORS}
          customColors={DEFAULT_CUSTOM_COLORS}
          updateColor={updateColorAndClose}
          updateCustomColor={updateColor}
        />
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

interface PureColorPickerProps extends React.ComponentProps<'div'> {
  colors: TColor[]
  customColors: TColor[]
  clearColor: () => void
  updateColor: (color: string) => void
  updateCustomColor: (color: string) => void
  color?: string
}

function PureColorPicker({
  className,
  clearColor,
  color,
  colors,
  customColors,
  updateColor,
  updateCustomColor,
  ...props
}: PureColorPickerProps) {
  return (
    <div className={cn('flex flex-col', className)} {...props}>
      <ToolbarMenuGroup label="Custom Colors">
        <ColorCustom
          color={color}
          className="px-2"
          colors={colors}
          customColors={customColors}
          updateColor={updateColor}
          updateCustomColor={updateCustomColor}
        />
      </ToolbarMenuGroup>
      <ToolbarMenuGroup label="Default Colors">
        <ColorDropdownMenuItems
          color={color}
          className="px-2"
          colors={colors}
          updateColor={updateColor}
        />
      </ToolbarMenuGroup>
      {color && (
        <ToolbarMenuGroup>
          <DropdownMenuItem className="p-2" onClick={clearColor}>
            <EraserIcon />
            <span>Clear</span>
          </DropdownMenuItem>
        </ToolbarMenuGroup>
      )}
    </div>
  )
}

const ColorPicker = React.memo(
  PureColorPicker,
  (prev, next) =>
    prev.color === next.color &&
    prev.colors === next.colors &&
    prev.customColors === next.customColors,
)

interface ColorCustomProps extends React.ComponentPropsWithoutRef<'div'> {
  colors: TColor[]
  customColors: TColor[]
  updateColor: (color: string) => void
  updateCustomColor: (color: string) => void
  color?: string
}

function ColorCustom({
  className,
  color,
  colors,
  customColors,
  updateColor,
  updateCustomColor,
  ...props
}: ColorCustomProps) {
  const [customColor, setCustomColor] = React.useState<string>()
  const [value, setValue] = React.useState<string>(color || '#000000')

  React.useEffect(() => {
    if (
      !color ||
      customColors.some((c) => c.value === color) ||
      colors.some((c) => c.value === color)
    ) {
      return
    }

    setCustomColor(color)
  }, [color, colors, customColors])

  const computedColors = React.useMemo(
    () =>
      customColor
        ? [
            ...customColors,
            {
              isBrightColor: false,
              name: '',
              value: customColor,
            },
          ]
        : customColors,
    [customColor, customColors],
  )

  const updateCustomColorDebounced = React.useMemo(
    () => debounce((nextColor: string) => updateCustomColor(nextColor), 100),
    [updateCustomColor],
  )

  React.useEffect(() => {
    return () => {
      updateCustomColorDebounced.cancel()
    }
  }, [updateCustomColorDebounced])

  return (
    <div className={cn('relative flex flex-col gap-4', className)} {...props}>
      <ColorDropdownMenuItems color={color} colors={computedColors} updateColor={updateColor}>
        <ColorInput
          value={value}
          onChange={(e) => {
            setValue(e.target.value)
            updateCustomColorDebounced(e.target.value)
          }}
        >
          <DropdownMenuItem
            className={cn(
              buttonVariants({
                size: 'icon',
                variant: 'outline',
              }),
              'absolute top-1 right-2 bottom-2 flex size-8 items-center justify-center rounded-full',
            )}
            onSelect={(e) => {
              e.preventDefault()
            }}
          >
            <span className="sr-only">Custom</span>
            <PlusIcon />
          </DropdownMenuItem>
        </ColorInput>
      </ColorDropdownMenuItems>
    </div>
  )
}

function ColorInput({
  children,
  className,
  value = '#000000',
  ...props
}: React.ComponentProps<'input'>) {
  const inputRef = React.useRef<HTMLInputElement | null>(null)

  return (
    <div className="flex flex-col items-center">
      {React.Children.map(children, (child) => {
        if (!child) return child

        return React.cloneElement(
          child as React.ReactElement<{
            onClick: () => void
          }>,
          {
            onClick: () => inputRef.current?.click(),
          },
        )
      })}
      <input
        {...props}
        ref={useComposedRef(props.ref, inputRef)}
        className={cn('size-0 overflow-hidden border-0 p-0', className)}
        value={value}
        type="color"
      />
    </div>
  )
}

interface ColorDropdownMenuItemProps extends React.ComponentProps<typeof DropdownMenuItem> {
  isBrightColor: boolean
  isSelected: boolean
  value: string
  updateColor: (color: string) => void
  name?: string
}

function ColorDropdownMenuItem({
  className,
  isBrightColor,
  isSelected,
  name,
  updateColor,
  value,
  ...props
}: ColorDropdownMenuItemProps) {
  const content = (
    <DropdownMenuItem
      className={cn(
        buttonVariants({
          size: 'icon',
          variant: 'outline',
        }),
        'my-1 flex size-6 items-center justify-center rounded-full border border-muted border-solid p-0 transition-all hover:scale-125',
        !isBrightColor && 'border-transparent',
        isSelected && 'border-2 border-primary',
        className,
      )}
      style={{ backgroundColor: value }}
      onSelect={(e) => {
        e.preventDefault()
        updateColor(value)
      }}
      {...props}
    />
  )

  return name ? (
    <Tooltip>
      <TooltipTrigger>{content}</TooltipTrigger>
      <TooltipContent className="mb-1 capitalize">{name}</TooltipContent>
    </Tooltip>
  ) : (
    content
  )
}

export interface ColorDropdownMenuItemsProps extends React.ComponentProps<'div'> {
  colors: TColor[]
  updateColor: (color: string) => void
  color?: string
}

export function ColorDropdownMenuItems({
  className,
  color,
  colors,
  updateColor,
  ...props
}: ColorDropdownMenuItemsProps) {
  return (
    <div
      className={cn('grid grid-cols-[repeat(10,1fr)] place-items-center gap-x-1', className)}
      {...props}
    >
      <TooltipProvider>
        {colors.map(({ isBrightColor, name, value }) => (
          <ColorDropdownMenuItem
            name={name}
            key={name ?? value}
            value={value}
            isBrightColor={isBrightColor}
            isSelected={color === value}
            updateColor={updateColor}
          />
        ))}
        {props.children}
      </TooltipProvider>
    </div>
  )
}

export { DEFAULT_COLORS }
