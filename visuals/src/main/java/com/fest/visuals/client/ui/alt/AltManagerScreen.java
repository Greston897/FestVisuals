package com.fest.visuals.client.ui.alt;

import net.minecraft.client.Minecraft;
import net.minecraft.client.User;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.EditBox;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.network.chat.Component;
import org.lwjgl.glfw.GLFW;

import java.util.Optional;
import java.util.UUID;
import java.util.List;

import java.awt.Color;

import com.fest.visuals.api.utils.render.RenderUtil;
import com.fest.visuals.api.utils.render.pipeline.FestRenderer;
import com.fest.visuals.client.ui.menu.MainMenuTheme;
import com.fest.visuals.inject.client.MixinMinecraftClientAccessor;

public class AltManagerScreen extends Screen {
    private final Screen parent;
    private EditBox nameField;
    private double scrollY = 0;

    public AltManagerScreen(Screen parent) {
        super(Component.literal("Alt Manager"));
        this.parent = parent;
    }

    @Override
    protected void init() {
        AltConfig.load();
        
        int centerX = this.width / 2;
        int centerY = this.height / 2;

        this.nameField = new EditBox(this.font, centerX - 120, centerY - 110, 240, 20, Component.literal("Nickname"));
        this.nameField.setMaxLength(16);
        this.addRenderableWidget(this.nameField);

        this.addRenderableWidget(Button.builder(Component.literal("\u0414\u043E\u0431\u0430\u0432\u0438\u0442\u044C \u0430\u043A\u043A\u0430\u0443\u043D\u0442"), button -> {
            String newName = nameField.getValue().trim();
            if (!newName.isEmpty()) {
                AltConfig.addAlt(newName);
            }
        }).bounds(centerX - 120, centerY - 85, 118, 20).build());

        this.addRenderableWidget(Button.builder(Component.literal("\u0420\u0430\u043D\u0434\u043E\u043C"), button -> {
            String chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
            StringBuilder sb = new StringBuilder();
            sb.append("FV_");
            for (int i = 0; i < 7; i++) {
                sb.append(chars.charAt((int)(Math.random() * chars.length())));
            }
            this.nameField.setValue(sb.toString());
        }).bounds(centerX + 2, centerY - 85, 118, 20).build());

        this.addRenderableWidget(Button.builder(Component.literal("\u041E\u0447\u0438\u0441\u0442\u0438\u0442\u044C \u0441\u043F\u0438\u0441\u043E\u043A \u0430\u043A\u043A\u0430\u0443\u043D\u0442\u043E\u0432"), button -> {
            AltConfig.getAlts().clear();
            AltConfig.save();
            scrollY = 0;
        }).bounds(centerX - 120, centerY + 85, 240, 20).build());

        this.addRenderableWidget(Button.builder(Component.literal("\u041D\u0430\u0437\u0430\u0434"), button -> {
            this.minecraft.gui.setScreen(this.parent);
        }).bounds(centerX - 120, centerY + 110, 240, 20).build());
    }

    private void login(String name) {
        User newUser = new User(name, UUID.randomUUID(), "", Optional.empty(), Optional.empty());
        ((MixinMinecraftClientAccessor) Minecraft.getInstance()).festvisuals$setUser(newUser);
    }

    @Override
    public void extractRenderState(GuiGraphicsExtractor extractor, int mouseX, int mouseY, float delta) {
        super.extractRenderState(extractor, mouseX, mouseY, delta);
        
        int centerX = this.width / 2;
        int centerY = this.height / 2;

        int listX = centerX - 120;
        int listY = centerY - 60;
        int listWidth = 240;
        int listHeight = 140;

        FestRenderer.withBackdrop(() -> MainMenuTheme.glass(RenderUtil.matrices(), listX, listY, listWidth, listHeight,
                10f, new Color(4, 4, 6, 235), 0.58f, 1f));

        List<String> alts = AltConfig.getAlts();
        double maxScroll = Math.max(0, alts.size() * 20 - listHeight + 4);
        if (scrollY > maxScroll) scrollY = maxScroll;
        if (scrollY < 0) scrollY = 0;

        extractor.enableScissor(listX, listY, listX + listWidth, listY + listHeight);

        String currentName = Minecraft.getInstance().getUser().getName();

        int yOffset = listY + 2 - (int)scrollY;
        for (int i = 0; i < alts.size(); i++) {
            String alt = alts.get(i);
            
            boolean isHovered = mouseX >= listX && mouseX <= listX + listWidth - 20 && mouseY >= yOffset && mouseY < yOffset + 20;
            boolean isDeleteHovered = mouseX >= listX + listWidth - 20 && mouseX <= listX + listWidth && mouseY >= yOffset && mouseY < yOffset + 20;

            int color = alt.equals(currentName) ? 0xFF00FF00 : (isHovered ? 0xFFDDDDDD : 0xFFFFFFFF);
            
            if (isHovered) {
                extractor.fill(listX, yOffset, listX + listWidth, yOffset + 20, 0x22FFFFFF);
            }

            extractor.text(this.font, alt, listX + 5, yOffset + 6, color);
            
            // Delete button (Red X)
            extractor.text(this.font, "x", listX + listWidth - 15, yOffset + 5, isDeleteHovered ? 0xFFFF0000 : 0xFFAAAAAA);

            yOffset += 20;
        }

        extractor.disableScissor();
    }

    @Override
    public boolean mouseScrolled(double mouseX, double mouseY, double scrollX, double scrollYEvent) {
        if (AltConfig.getAlts().size() * 20 > 140) {
            scrollY -= scrollYEvent * 20;
            return true;
        }
        return super.mouseScrolled(mouseX, mouseY, scrollX, scrollYEvent);
    }

    @Override
    public boolean mouseClicked(MouseButtonEvent event, boolean wasHandled) {
        if (event.button() == 0) {
            int centerX = this.width / 2;
            int centerY = this.height / 2;
            int listX = centerX - 120;
            int listY = centerY - 60;
            int listWidth = 240;
            int listHeight = 140;

            double mx = event.x();
            double my = event.y();

            if (mx >= listX && mx <= listX + listWidth && my >= listY && my <= listY + listHeight) {
                List<String> alts = AltConfig.getAlts();
                int yOffset = listY + 2 - (int)scrollY;
                for (int i = 0; i < alts.size(); i++) {
                    if (my >= yOffset && my < yOffset + 20) {
                        if (mx >= listX + listWidth - 20) {
                            // Delete
                            AltConfig.removeAlt(alts.get(i));
                            return true;
                        } else {
                            // Login
                            login(alts.get(i));
                            return true;
                        }
                    }
                    yOffset += 20;
                }
            }
        }
        return super.mouseClicked(event, wasHandled);
    }

    @Override
    public boolean keyPressed(KeyEvent event) {
        if (event.key() == GLFW.GLFW_KEY_ESCAPE) {
            this.minecraft.gui.setScreen(this.parent);
            return true;
        }
        return super.keyPressed(event);
    }
}
