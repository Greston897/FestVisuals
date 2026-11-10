package com.fest.visuals.client.ui.theme;

import lombok.Getter;
import lombok.Setter;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import org.lwjgl.glfw.GLFW;
import com.fest.visuals.api.event.Listener;
import com.fest.visuals.api.event.events.other.WindowResizeEvent;
import com.fest.visuals.api.system.configs.ThemeManager;
import com.fest.visuals.api.utils.animation.AnimationUtil;
import com.fest.visuals.api.utils.animation.Easing;
import com.fest.visuals.api.utils.color.ColorUtil;
import com.fest.visuals.api.utils.color.UIColors;
import com.fest.visuals.api.utils.math.MouseUtil;
import com.fest.visuals.api.utils.render.RenderUtil;
import com.fest.visuals.api.utils.render.ScissorUtil;
import com.fest.visuals.api.utils.render.fonts.Fonts;
import com.fest.visuals.api.utils.render.fonts.Icons;
import com.fest.visuals.client.ui.UIComponent;
import com.fest.visuals.client.ui.clickgui.module.settings.ColorComponent;
import com.mojang.blaze3d.vertex.PoseStack;
import java.util.ArrayList;
import java.util.List;

@Getter
public class ThemeEditor extends UIComponent {
    @Getter private static final ThemeEditor instance = new ThemeEditor();

    private final List<ThemeSelectable> themeSelectables = new ArrayList<>();
    private final List<ThemeBound> themeBounds = new ArrayList<>();

    private final Theme defaultTheme = new Theme("Wedal");
    private Theme currentTheme = defaultTheme;
    protected Theme editTheme;

    private final String placeHolderText = "Enter name...";
    private String typingText = "";
    private boolean typing;

    private DeleteButton deleteButton = new DeleteButton(-1f, -1f, -1f);

    private final AnimationUtil openAnimation = new AnimationUtil();
    private float scroll;
    private float contentHeight;
    private final AnimationUtil scrollAnimation = new AnimationUtil();
    @Setter private boolean open;
    @Setter private boolean embedded;
    @Setter private float anim = 1f;

    private int alphaAnim() {
        return (int) (net.minecraft.util.Mth.clamp(openAnimation.getValue() * anim, 0.0, 1.0) * 255);
    }

    private record ThemeBound(float x, float y, float width, float height, Theme.ElementColor elementColor) { }
    private record DeleteButton(float x, float y, float size) {}

    public ThemeEditor() {
        setWidth(scaled(95f));
        setHeight(scaled(150f));

        WindowResizeEvent.getInstance().subscribe(new Listener<>(-1, event -> {
            setWidth(scaled(95f));
        }));
    }

    public void init() {
        refresh();
    }

    private void refresh() {
        ThemeManager.getInstance().refresh();
    }

    /** Names of the saved themes, for voice control. */
    public List<String> themeNames() {
        List<String> names = new ArrayList<>();
        for (ThemeSelectable selectable : themeSelectables) names.add(selectable.getTheme().getName());
        return names;
    }

    /** Switches to the theme with this name (case-insensitive) and remembers it, as a click would. */
    public boolean selectTheme(String name) {
        for (ThemeSelectable selectable : themeSelectables) {
            if (selectable.getTheme().getName().equalsIgnoreCase(name.trim())) {
                currentTheme = selectable.getTheme();
                ThemeManager.getInstance().saveLastSelected(currentTheme);
                return true;
            }
        }
        return false;
    }

    public void save(boolean last) {
        if (!last) {
            ThemeManager.getInstance().saveAll();
        } else if (currentTheme != null) {
            ThemeManager.getInstance().saveLastSelected(currentTheme);
        }
    }

    public void load() {
        ThemeManager.getInstance().refresh();
        Theme last = ThemeManager.getInstance().loadLastSelected();
        if (last != null) {
            currentTheme = last;
            // sync last-selected with the entry inside the refreshed list (identity match by name)
            for (ThemeSelectable ts : themeSelectables) {
                if (ts.getTheme().getName().equalsIgnoreCase(last.getName())) {
                    currentTheme = ts.getTheme();
                    break;
                }
            }
        } else if (!themeSelectables.isEmpty()) {
            currentTheme = themeSelectables.get(0).getTheme();
        } else {
            themeSelectables.add(new ThemeSelectable(defaultTheme));
            currentTheme = defaultTheme;
        }
    }

    @Override
    public void render(GuiGraphicsExtractor context, int mouseX, int mouseY, float delta) {
        openAnimation.update();
        openAnimation.run(open ? 1.0 : 0.0, 100, Easing.SINE_OUT);
        if (openAnimation.getValue() <= 0.1) return;

        themeBounds.clear();

        //System.out.println(currentTheme.getName());
        //for (Theme.ElementColor elementColor : currentTheme.getElementColors()) { System.out.println(elementColor.getName() + ": " + elementColor.getColor()); }

        float round = getWidth() * 0.05f;
        float headerHeight = scaled(getHeaderHeight());
        float headerFontSize = headerHeight * 0.52f;
        String text = "Theme Editor";
        float textWidth = Fonts.PS_BOLD.getWidth(text, headerFontSize);
        if (!embedded) {
            setWidth(textWidth * 1.5f);
        }

        PoseStack matrixStack = RenderUtil.matrices();

        RenderUtil.BLUR_RECT.draw(matrixStack, getX(), getY(), getWidth(), getHeight(), round, UIColors.blur(alphaAnim()));
        Fonts.PS_BOLD.drawGradientText(matrixStack, text, getX() + getWidth() / 2f - textWidth / 2f, getY() + headerHeight / 2f - headerFontSize / 2f, headerFontSize, UIColors.primary(alphaAnim()), UIColors.secondary(alphaAnim()), Fonts.PS_BOLD.getWidth(text, headerFontSize) / 4f);

        placeRender(context, mouseX, mouseY, delta);

        float xOffset = getX() + offset();
        float widthOffset = getWidth() - offset() * 2f;

        scrollAnimation.update();
        scrollAnimation.run(scroll, 600, Easing.EXPO_OUT);
        float maxScroll = Math.max(0, contentHeight - getHeight());
        scroll = com.fest.visuals.api.utils.math.MathUtil.interpolate(scroll, net.minecraft.util.Mth.clamp(scroll, -maxScroll, 0), 0.5f);
        float themeY = gap() + headerHeight + getPlaceTextCoordinates()[3] + (float)scrollAnimation.getValue();

        float listTop = getY() + headerHeight + getPlaceTextCoordinates()[3] + gap();
        float listHeight = getY() + getHeight() - listTop;
        ScissorUtil.start(matrixStack, getX(), listTop, getWidth(), listHeight);

        if (editTheme == null) {
            for (ThemeSelectable theme : themeSelectables) {
                theme.setAlpha((float) (openAnimation.getValue() * anim));
                theme.setX(xOffset);
                theme.setY(getY() + themeY);
                theme.setWidth(widthOffset);
                theme.setHeight(scaled(17f));

                theme.render(context, mouseX, mouseY, delta);

                themeY += theme.getHeight() + gap();
            }

            contentHeight = themeY - (float)scrollAnimation.getValue();
        } else {
            float elementY = themeY;
            float elementHeight = scaled(getElementHeight());

            float textSize = elementHeight * 0.6f;
            float textY = getY() + elementHeight / 2f - textSize / 2f;

            float elementWidth = getWidth() - offset();

            float colorSize = elementHeight * 0.8f;
            float colorX = getX() + elementWidth - colorSize;
            float colorY = getY() + elementHeight / 2f - colorSize / 2f;

            float roundColor = colorSize * 0.2f;

            for (Theme.ElementColor elementColor : editTheme.getElementColors()) {
                float height = elementHeight;
                Fonts.PS_MEDIUM.drawText(matrixStack, elementColor.getName(), xOffset, textY + elementY, textSize, UIColors.textColor(alphaAnim()));
                RenderUtil.RECT.draw(matrixStack, colorX, colorY + elementY, colorSize, colorSize, roundColor, ColorUtil.setAlpha(elementColor.getColor(), alphaAnim()));

                ColorComponent colorComponent = elementColor.getColorComponent();
                colorComponent.updateOpen();
                float cAnim = colorComponent.getValue();

                if (cAnim > 0.0) {
                    colorComponent.setX(xOffset);
                    colorComponent.setY(textY + elementY + elementHeight);
                    colorComponent.setWidth(widthOffset);
                    colorComponent.setAlpha(alphaAnim() / 255f);
                    colorComponent.render(context, mouseX, mouseY, delta);

                    height += colorComponent.getHeight() + (gap() * 2f) * cAnim;
                }

                themeBounds.add(new ThemeBound(xOffset, colorY + elementY, elementWidth, height, elementColor));

                elementY += height + gap();
            }

            contentHeight = elementY - (float)scrollAnimation.getValue();
        }

        ScissorUtil.stop(matrixStack);
    }

    private void placeRender(GuiGraphicsExtractor context, int mouseX, int mouseY, float delta) {
        PoseStack matrixStack = RenderUtil.matrices();

        float[] placeCoords = getPlaceTextCoordinates();
        float placeX = placeCoords[0];
        float placeY = placeCoords[1];
        float placeWidth = placeCoords[2];
        float placeHeight = placeCoords[3];
        float saveWidth = scaled(40f);
        float placeFontSize = placeHeight * 0.4f;
        float placeRound = placeHeight * 0.2f;

        String cursor = typing && System.currentTimeMillis() % 1000 > 500 ? "_" : " ";
        String typeText = typingText.isEmpty() && !typing ? placeHolderText : typingText + cursor;

        boolean edit = editTheme != null;

        if (edit) {
            typeText = editTheme.getName();
        }

        RenderUtil.RECT.draw(matrixStack, placeX, placeY, edit ? placeWidth : placeWidth - saveWidth - gap(), placeHeight, placeRound, UIColors.widgetBlur(alphaAnim()));
        if (!edit) {
            RenderUtil.RECT.draw(matrixStack, placeX + placeWidth - saveWidth, placeY, saveWidth, placeHeight, placeRound, UIColors.primary(alphaAnim()));
            Fonts.PS_BOLD.drawCenteredText(matrixStack, "Create", placeX + placeWidth - saveWidth / 2f, placeY + placeHeight / 2f - placeFontSize / 2f, placeFontSize, ColorUtil.setAlpha(java.awt.Color.WHITE, (int)alphaAnim()));
        }
        Fonts.PS_BOLD.drawText(matrixStack, typeText, placeX + offset(), placeY + placeHeight / 2f - placeFontSize / 2f, placeFontSize, UIColors.textColor(alphaAnim()));

        if (edit && themeSelectables.size() > 1) {
            float margin = gap();
            float deleteSize = placeHeight - margin * 2f;
            float deleteX = placeX + getWidth() - deleteSize - margin - offset() * 2f;
            float deleteY = placeY + margin;
            float iconSize = deleteSize * 0.5f;
            RenderUtil.RECT.draw(matrixStack, deleteX, deleteY, deleteSize, deleteSize, placeRound, UIColors.blur(alphaAnim()));
            Fonts.ICONS.drawCenteredText(matrixStack, Icons.CROSS.getLetter(), deleteX + deleteSize / 2f, deleteY + deleteSize / 2f - iconSize / 2f, iconSize, UIColors.textColor(alphaAnim()), 0.1f);

            deleteButton = new DeleteButton(deleteX, deleteY, deleteSize);
        }
    }

    @Override
    public void keyPressed(int keyCode, int scanCode, int modifiers) {
        if (!open) return;

        if (typing && editTheme == null) {
            switch (keyCode) {
                case GLFW.GLFW_KEY_BACKSPACE -> {
                    if (!typingText.isEmpty()) {
                        typingText = typingText.substring(0, typingText.length() - 1);
                    }
                }

                case GLFW.GLFW_KEY_ENTER -> {
                    if (!typingText.isEmpty()) {
                        boolean exists = themeSelectables.stream().anyMatch(ts -> ts.getTheme().getName().equalsIgnoreCase(typingText));

                        if (!exists) {
                            Theme newTheme = new Theme(ThemeManager.getInstance().safeFileName(typingText));
                            newTheme.getElementColors().clear();
                            for (Theme.ElementColor elementColor : currentTheme.getElementColors()) {
                                newTheme.getElementColors().add(new Theme.ElementColor(
                                        elementColor.getName(),
                                        elementColor.getColor()
                                ));
                            }

                            themeSelectables.add(new ThemeSelectable(newTheme));
                            currentTheme = newTheme;
                        }

                        typingText = "";
                        typing = false;
                    }
                }
            }
        }
    }

    @Override
    public void mouseClicked(double mouseX, double mouseY, int button) {
        if (!open) return;

        if (editTheme != null) {
            if (themeSelectables.size() > 1 && MouseUtil.isHovered(mouseX, mouseY, deleteButton.x, deleteButton.y, deleteButton.size, deleteButton.size)) {

                if (themeSelectables.removeIf(ts -> ts.getTheme() == editTheme)) {
                    ThemeManager.getInstance().remove(editTheme.getName());
                    editTheme = null;
                }

                return;
            }

            if (MouseUtil.isHovered(mouseX, mouseY, getX(), getY(), getWidth(), scaled(getHeaderHeight()))) {
                editTheme = null;
                return;
            }

            for (ThemeBound themeBound : themeBounds) {
                ColorComponent colorComponent = themeBound.elementColor.getColorComponent();
                if (MouseUtil.isHovered(mouseX, mouseY, themeBound.x, themeBound.y, themeBound.width, scaled(getElementHeight()))) {
                    colorComponent.toggleOpen();
                    return;
                }
                if (MouseUtil.isHovered(mouseX, mouseY, themeBound.x, themeBound.y, themeBound.width, themeBound.height)) {
                    colorComponent.mouseClicked(mouseX, mouseY, button);
                }
            }
        } else {
            float[] coords = getPlaceTextCoordinates();
            float saveW = scaled(40f);
            if (MouseUtil.isHovered(mouseX, mouseY, coords[0] + coords[2] - saveW, coords[1], saveW, coords[3])) {
                if (!typingText.isEmpty()) {
                    Theme newTheme = new Theme(typingText);
                    ThemeManager.getInstance().save(newTheme);
                    themeSelectables.add(new ThemeSelectable(newTheme));
                    typingText = "";
                    typing = false;
                }
                return;
            }
            if (MouseUtil.isHovered(mouseX, mouseY, coords[0], coords[1], coords[2] - saveW - gap(), coords[3])) {
                typing = !typing;
                return;
            }
            for (ThemeSelectable theme : themeSelectables) {
                if (MouseUtil.isHovered(mouseX, mouseY, theme.getX(), theme.getY(), theme.getWidth(), theme.getHeight())) {
                    if (button == 1) {
                        editTheme = theme.getTheme();
                    } else if (button == 0) {
                        currentTheme = theme.getTheme();
                        ThemeManager.getInstance().saveLastSelected(currentTheme);
                    }
                }
            }
        }
    }

    @Override
    public void mouseReleased(double mouseX, double mouseY, int button) {
        if (!open) return;

        if (editTheme != null) {
            for (ThemeBound themeBound : themeBounds) {
                themeBound.elementColor.getColorComponent().mouseReleased(mouseX, mouseY, button);
            }
        }
    }

    public boolean charTyped(char chr, int modifiers) {
        if (!open) return false;

        if (typing && typingText.length() < 10) {
            typingText += chr;
            
        }
        return false;
    }

    private float[] getPlaceTextCoordinates() {
        float x = getX() + offset();
        float y = getY() + scaled(getHeaderHeight());
        float width = getWidth() - offset() * 2f;
        float height = scaled(getTypingFieldHeight());
        return new float[]{x, y, width, height};
    }
    private float getElementHeight() {
        return 12f;
    }
    private float getHeaderHeight() {
        return 19f;
    }
    private float getTypingFieldHeight() {
        return 19f;
    }

    @Override
    public void mouseScrolled(double mouseX, double mouseY, double horizontalAmount, double verticalAmount) {
        if (!open) return;
        scroll += verticalAmount * 20.0;
        
    }
    
    public void mouseScrolledOld(double mouseX, double mouseY, double horizontalAmount, double verticalAmount) {

    }
}
